use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use scoop_identity as identity;

use super::*;

/// Normalize every C boundary used by this module while the source exact-type
/// relation is still available. LIR's physical `CType` deliberately does not
/// carry enough source semantics to reconstruct these records later.
pub(super) fn lower(module: &mir::Module, structs: &lir::StructDefs) -> lir::CanonicalCAbiMetadata {
    let mut builder = CanonicalCAbiBuilder::new(module, structs);

    for (_, external) in module.extern_functions.iter() {
        if external.abi == mir::ExternAbi::C {
            builder.add_signature(&external.params, &external.return_type);
        }
    }
    for (_, global) in module.globals.iter() {
        if matches!(global.storage, mir::GlobalStorage::Extern { .. }) {
            builder.storage(&global.ty);
        }
    }
    for (_, callback) in module.callback_bridges.iter() {
        builder.add_function_type(callback.signature);
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        builder.add_function_type(callback.native_signature);
    }

    builder.finish()
}

struct CanonicalCAbiBuilder<'module> {
    module: &'module mir::Module,
    structs: &'module lir::StructDefs,
    signatures: Vec<identity::CanonicalCAbiSignatureFingerprintRecord>,
    layouts: Vec<identity::CanonicalCAbiLayoutFingerprintRecord>,
    layout_fingerprints: HashMap<mir::StructId, identity::CanonicalCAbiLayoutFingerprint>,
    visiting_layouts: HashSet<mir::StructId>,
}

impl<'module> CanonicalCAbiBuilder<'module> {
    fn new(module: &'module mir::Module, structs: &'module lir::StructDefs) -> Self {
        Self {
            module,
            structs,
            signatures: Vec::new(),
            layouts: Vec::new(),
            layout_fingerprints: HashMap::new(),
            visiting_layouts: HashSet::new(),
        }
    }

    fn finish(self) -> lir::CanonicalCAbiMetadata {
        lir::CanonicalCAbiMetadata::checked(self.signatures, self.layouts)
            .expect("canonical C ABI hashes must not collide")
    }

    fn add_function_type(&mut self, id: mir::FunctionTypeId) {
        let signature = &self.module.function_types[id];
        self.add_signature(&signature.parameter_types, &signature.return_type);
    }

    fn add_signature(&mut self, parameter_types: &[mir::Type], return_type: &mir::Type) {
        let parameters = parameter_types
            .iter()
            .map(|ty| {
                let storage = self.storage(ty);
                identity::CanonicalCAbiParameter::new(storage.exact_type(), storage)
                    .expect("canonical parameter storage retains its source exact type")
            })
            .collect();
        let result = if return_type == &mir::Type::Unit {
            identity::CanonicalCAbiReturn::Void
        } else {
            let storage = self.storage(return_type);
            identity::CanonicalCAbiReturn::value(storage.exact_type(), storage)
                .expect("canonical return storage retains its source exact type")
        };
        let signature = identity::CanonicalCAbiFunctionSignature::cdecl(parameters, result);
        self.signatures.push(
            identity::CanonicalCAbiSignatureFingerprintRecord::new(signature)
                .expect("canonical C ABI signatures have encodable identities"),
        );
    }

    fn storage(&mut self, ty: &mir::Type) -> identity::CanonicalCStorageType {
        match ty {
            mir::Type::Unit => unreachable!("Unit has no C object representation"),
            mir::Type::Integer(kind) => {
                let exact_type = self.exact_type(ty);
                identity::CanonicalCStorageType::Integer {
                    exact_type,
                    signedness: match kind.signedness() {
                        mir::IntegerSignedness::Signed => identity::Signedness::Signed,
                        mir::IntegerSignedness::Unsigned => identity::Signedness::Unsigned,
                    },
                    bit_width: match kind.width() {
                        mir::IntegerWidth::W8 => identity::IntegerBitWidth::Bits8,
                        mir::IntegerWidth::W16 => identity::IntegerBitWidth::Bits16,
                        mir::IntegerWidth::W32 => identity::IntegerBitWidth::Bits32,
                        mir::IntegerWidth::W64 => identity::IntegerBitWidth::Bits64,
                    },
                }
            }
            mir::Type::Boolean => identity::CanonicalCStorageType::Boolean {
                exact_type: self.exact_type(ty),
            },
            mir::Type::Ptr(pointee) => identity::CanonicalCStorageType::DataPointer {
                exact_type: self.exact_type(ty),
                pointee: self.data_pointee(pointee),
                storage: identity::CPointerStorage::Direct,
            },
            mir::Type::FunPtr(_) => identity::CanonicalCStorageType::CodePointer {
                exact_type: self.exact_type(ty),
                storage: identity::CPointerStorage::Direct,
            },
            mir::Type::Struct(id) => identity::CanonicalCStorageType::Struct {
                exact_type: self.exact_type(ty),
                layout: self.layout(*id),
            },
            mir::Type::Enum(id, arguments) if self.module.option_core(*id).is_some() => {
                let exact_type = self.exact_type(ty);
                match exact_option_payload(self.module, *id, arguments) {
                    mir::Type::Ptr(pointee) => identity::CanonicalCStorageType::DataPointer {
                        exact_type,
                        pointee: self.data_pointee(pointee),
                        storage: identity::CPointerStorage::NullableWrapper(exact_type),
                    },
                    mir::Type::FunPtr(_) => identity::CanonicalCStorageType::CodePointer {
                        exact_type,
                        storage: identity::CPointerStorage::NullableWrapper(exact_type),
                    },
                    other => unreachable!(
                        "validated core Option payload {} is not C-nullable",
                        mir::type_name(self.module, other)
                    ),
                }
            }
            mir::Type::MachineScalar(kind) => {
                unreachable!("internal machine scalar {kind:?} cannot cross source C FFI")
            }
            other => unreachable!(
                "HIR C-FFI classification rejects {} before MIR",
                mir::type_name(self.module, other)
            ),
        }
    }

    fn data_pointee(&self, pointee: &mir::Type) -> identity::CDataPointee {
        if pointee == &mir::Type::Unit {
            identity::CDataPointee::OpaqueUnit
        } else {
            identity::CDataPointee::ExactObject(self.exact_type(pointee))
        }
    }

    fn layout(&mut self, id: mir::StructId) -> identity::CanonicalCAbiLayoutFingerprint {
        if let Some(fingerprint) = self.layout_fingerprints.get(&id) {
            return *fingerprint;
        }
        assert!(
            self.visiting_layouts.insert(id),
            "validated C layouts cannot contain a by-value cycle"
        );

        let ty = mir::Type::Struct(id);
        let exact_type = self.exact_type(&ty);
        let definition = &self.structs[struct_def_id(id)];
        let contract = definition
            .c_layout()
            .expect("C ABI storage only admits C-layout structs");
        let source_fields = self.module.structs[id].declared_fields();
        let physical_fields = definition
            .c_fields()
            .expect("a C-layout LIR struct retains physical C fields");
        assert_eq!(
            source_fields.len(),
            physical_fields.len(),
            "C-layout source and physical field sequences must align"
        );
        let fields = source_fields
            .iter()
            .zip(physical_fields)
            .map(|(source, physical)| {
                assert_eq!(
                    source.identity, physical.identity,
                    "C-layout field identity survives MIR to LIR lowering"
                );
                identity::CanonicalCAbiLayoutField::new(
                    source.identity,
                    physical.layout.offset,
                    self.storage(&source.ty),
                )
            })
            .collect();
        let layout = identity::CanonicalCAbiLayout::new(
            exact_type,
            definition.size,
            NonZeroU64::new(definition.align).expect("a physical C layout has nonzero alignment"),
            layout_override(contract.aligned),
            layout_override(contract.packed),
            fields,
        );
        let record = identity::CanonicalCAbiLayoutFingerprintRecord::new(layout)
            .expect("canonical C ABI layouts have encodable identities");
        let fingerprint = record.fingerprint();
        self.layouts.push(record);
        self.layout_fingerprints.insert(id, fingerprint);
        assert!(self.visiting_layouts.remove(&id));
        fingerprint
    }

    fn exact_type(&self, ty: &mir::Type) -> identity::PersistentExactTypeId {
        exact_type_record(self.module, ty).id()
    }
}

fn layout_override(value: lir::LirCLayoutValue) -> identity::CLayoutOverride {
    match value {
        lir::LirCLayoutValue::Natural => identity::CLayoutOverride::Natural,
        lir::LirCLayoutValue::A1 => {
            identity::CLayoutOverride::Bytes(identity::CLayoutByteAlignment::Bytes1)
        }
        lir::LirCLayoutValue::A2 => {
            identity::CLayoutOverride::Bytes(identity::CLayoutByteAlignment::Bytes2)
        }
        lir::LirCLayoutValue::A4 => {
            identity::CLayoutOverride::Bytes(identity::CLayoutByteAlignment::Bytes4)
        }
        lir::LirCLayoutValue::A8 => {
            identity::CLayoutOverride::Bytes(identity::CLayoutByteAlignment::Bytes8)
        }
        lir::LirCLayoutValue::A16 => {
            identity::CLayoutOverride::Bytes(identity::CLayoutByteAlignment::Bytes16)
        }
    }
}
