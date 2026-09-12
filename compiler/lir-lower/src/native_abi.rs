use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use scoop_identity as identity;

use super::*;

type NativeLinkRequirementRecord = identity::CborIdentityRecord<
    identity::NativeLinkRequirementId,
    identity::NativeLinkRequirementKey,
>;

pub(super) struct LoweredNativeAbi {
    pub(super) canonical_c_abi: lir::CanonicalCAbiMetadata,
    pub(super) native_externals: lir::NativeExternalMetadata,
    pub(super) callback_signatures:
        HashMap<mir::FunctionTypeId, identity::CanonicalCAbiSignatureFingerprint>,
}

/// Normalize every C boundary used by this module while the source exact-type
/// relation is still available. LIR's physical `CType` deliberately does not
/// carry enough source semantics to reconstruct these records later.
pub(super) fn lower(
    context: &LoweringContext,
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> LoweredNativeAbi {
    let mut builder = CanonicalCAbiBuilder::new(context.target_profile(), module, structs, enums);

    for (_, external) in module.extern_functions.iter() {
        builder.add_external_function(context, external);
    }
    for (_, global) in module.globals.iter() {
        if let mir::GlobalStorage::Extern {
            source_contract,
            thread_local,
            ..
        } = &global.storage
        {
            builder.add_external_global(source_contract, global, *thread_local);
        }
    }
    for (_, callback) in module.callback_bridges.iter() {
        builder.add_function_type(callback.signature);
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        builder.add_callback_signature(callback.native_signature);
    }

    builder.finish()
}

struct CanonicalCAbiBuilder<'module> {
    module: &'module mir::Module,
    structs: &'module lir::StructDefs,
    enums: &'module lir::EnumDefs,
    target_profile: lir::LirTargetProfile,
    signatures: Vec<identity::CanonicalCAbiSignatureFingerprintRecord>,
    layouts: Vec<identity::CanonicalCAbiLayoutFingerprintRecord>,
    contracts: Vec<identity::NativeExternalContractRecord>,
    link_requirements: Vec<NativeLinkRequirementRecord>,
    callback_signatures: HashMap<mir::FunctionTypeId, identity::CanonicalCAbiSignatureFingerprint>,
    layout_fingerprints: HashMap<mir::StructId, identity::CanonicalCAbiLayoutFingerprint>,
    visiting_layouts: HashSet<mir::StructId>,
}

impl<'module> CanonicalCAbiBuilder<'module> {
    fn new(
        target_profile: lir::LirTargetProfile,
        module: &'module mir::Module,
        structs: &'module lir::StructDefs,
        enums: &'module lir::EnumDefs,
    ) -> Self {
        Self {
            module,
            structs,
            enums,
            target_profile,
            signatures: Vec::new(),
            layouts: Vec::new(),
            contracts: Vec::new(),
            link_requirements: Vec::new(),
            callback_signatures: HashMap::new(),
            layout_fingerprints: HashMap::new(),
            visiting_layouts: HashSet::new(),
        }
    }

    fn finish(self) -> LoweredNativeAbi {
        LoweredNativeAbi {
            canonical_c_abi: lir::CanonicalCAbiMetadata::checked(self.signatures, self.layouts)
                .expect("canonical C ABI hashes must not collide"),
            native_externals: lir::NativeExternalMetadata::checked(
                self.contracts,
                self.link_requirements,
            )
            .expect("validated source externs normalize to one target contract each"),
            callback_signatures: self.callback_signatures,
        }
    }

    fn add_external_function(&mut self, context: &LoweringContext, external: &mir::ExternFunction) {
        let (symbol, source_library) = source_target(external.source_contract.contract());
        let symbol_key = self.target_symbol(symbol);
        let library = self.library_binding(source_library);
        let contract = match external.abi {
            mir::ExternAbi::C => identity::NativeExternalContract::c_function(
                library,
                self.add_signature(&external.params, &external.return_type),
            ),
            mir::ExternAbi::Scoop => {
                let physical = abi::classify_mir_signature(
                    context,
                    external.params.iter(),
                    &external.return_type,
                    self.structs,
                    self.enums,
                );
                identity::NativeExternalContract::scoop_function(
                    library,
                    self.scoop_signature(
                        &external.params,
                        &external.return_type,
                        external.gc_effect,
                        &physical,
                    ),
                    identity::TargetCallingConvention::Cdecl,
                )
            }
        };
        self.contracts.push(
            identity::NativeExternalContractRecord::new(
                external.source_contract.id(),
                symbol_key,
                contract,
            )
            .expect("validated target-native contracts have encodable identities"),
        );
    }

    fn add_external_global(
        &mut self,
        source_contract: &identity::SourceNativeExternalContractRecord,
        global: &mir::Global,
        thread_local: bool,
    ) {
        let (symbol, source_library) = source_target(source_contract.contract());
        let symbol_key = self.target_symbol(symbol);
        let library = self.library_binding(source_library);
        let storage = self.storage(&global.ty);
        let contract = match (global.mutable, thread_local) {
            (false, false) => identity::NativeExternalContract::read_only_data(library, storage),
            (true, false) => identity::NativeExternalContract::mutable_data(library, storage),
            (false, true) => identity::NativeExternalContract::read_only_tls(library, storage),
            (true, true) => identity::NativeExternalContract::mutable_tls(library, storage),
        };
        self.contracts.push(
            identity::NativeExternalContractRecord::new(source_contract.id(), symbol_key, contract)
                .expect("validated target-native contracts have encodable identities"),
        );
    }

    fn target_symbol(
        &self,
        symbol: &identity::SourceNativeSymbol,
    ) -> identity::NativeExternalSymbolKey {
        match self.target_profile.id() {
            lir::TargetProfileId::DarwinAarch64 => {
                identity::NativeExternalSymbolKey::darwin_macho_external(symbol)
                    .expect("validated source native symbols normalize for Mach-O")
            }
        }
    }

    fn library_binding(
        &mut self,
        source: &identity::SourceNativeLibraryBinding,
    ) -> identity::NativeLibraryBinding {
        match source {
            identity::SourceNativeLibraryBinding::DefaultNativeNamespace => {
                identity::NativeLibraryBinding::DefaultNativeNamespace
            }
            identity::SourceNativeLibraryBinding::LogicalLibrary(name) => {
                let record = NativeLinkRequirementRecord::from_key(
                    identity::NativeLinkRequirementKey::target_default(name.clone()),
                )
                .expect("validated native library names have encodable requirements");
                self.link_requirements.push(record.clone());
                identity::NativeLibraryBinding::Requirement(record.id())
            }
        }
    }

    fn add_function_type(&mut self, id: mir::FunctionTypeId) {
        let signature = &self.module.function_types[id];
        self.add_signature(&signature.parameter_types, &signature.return_type);
    }

    fn add_callback_signature(&mut self, id: mir::FunctionTypeId) {
        let signature = &self.module.function_types[id];
        let record = self.signature_record(&signature.parameter_types, &signature.return_type);
        let fingerprint = record.fingerprint();
        self.signatures.push(record);
        let previous = self.callback_signatures.insert(id, fingerprint);
        assert!(
            previous.is_none_or(|previous| previous == fingerprint),
            "one MIR function type has one canonical C ABI signature"
        );
    }

    fn add_signature(
        &mut self,
        parameter_types: &[mir::Type],
        return_type: &mir::Type,
    ) -> identity::CanonicalCAbiFunctionSignature {
        let record = self.signature_record(parameter_types, return_type);
        let signature = record.signature().clone();
        self.signatures.push(record);
        signature
    }

    fn signature_record(
        &mut self,
        parameter_types: &[mir::Type],
        return_type: &mir::Type,
    ) -> identity::CanonicalCAbiSignatureFingerprintRecord {
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
        identity::CanonicalCAbiSignatureFingerprintRecord::new(
            identity::CanonicalCAbiFunctionSignature::cdecl(parameters, result),
        )
        .expect("canonical C ABI signatures have encodable identities")
    }

    fn scoop_signature(
        &self,
        parameter_types: &[mir::Type],
        return_type: &mir::Type,
        gc_effect: mir::GcEffect,
        physical: &lir::ScoopAbiSignature,
    ) -> identity::CanonicalScoopAbiFunctionSignature {
        assert_eq!(
            parameter_types.len(),
            physical.arguments().len(),
            "one physical Scoop ABI convention exists per logical parameter"
        );
        let parameters = parameter_types
            .iter()
            .map(|ty| self.exact_type(ty))
            .collect::<Vec<_>>();
        let result = self.exact_type(return_type);
        let exact_signature = identity::ExactCallableSignature::new(
            identity::Effect::Ordinary,
            None,
            parameters.clone(),
            result,
        );
        let arguments = parameters
            .into_iter()
            .zip(physical.arguments())
            .map(|(exact_type, argument)| match argument {
                lir::AbiArgument::ElidedZst(value) => {
                    identity::ScoopAbiArgument::elided_zst(self.scoop_storage(
                        exact_type,
                        value.storage_type(),
                        value.layout().size(),
                        value.layout().alignment(),
                    ))
                }
                lir::AbiArgument::Direct(value) => {
                    identity::ScoopAbiArgument::direct(self.scoop_storage(
                        exact_type,
                        value.storage_type(),
                        value.layout().size().get(),
                        value.layout().alignment(),
                    ))
                }
                lir::AbiArgument::Indirect(value) => {
                    identity::ScoopAbiArgument::indirect(self.scoop_storage(
                        exact_type,
                        value.storage_type(),
                        value.layout().size().get(),
                        value.layout().alignment(),
                    ))
                }
            })
            .map(|argument| {
                argument.expect("LIR Scoop ABI passing agrees with canonical storage shape")
            })
            .collect();
        let result = match physical.result() {
            lir::AbiReturn::UnitVoid => identity::ScoopAbiReturn::unit_void(),
            lir::AbiReturn::ElidedZst(value) => {
                identity::ScoopAbiReturn::elided_zst(self.scoop_storage(
                    result,
                    value.storage_type(),
                    value.layout().size(),
                    value.layout().alignment(),
                ))
                .expect("LIR Scoop ABI passing agrees with canonical storage shape")
            }
            lir::AbiReturn::Direct(value) => identity::ScoopAbiReturn::direct(self.scoop_storage(
                result,
                value.storage_type(),
                value.layout().size().get(),
                value.layout().alignment(),
            ))
            .expect("LIR Scoop ABI passing agrees with canonical storage shape"),
            lir::AbiReturn::Indirect(value) => {
                identity::ScoopAbiReturn::indirect(self.scoop_storage(
                    result,
                    value.storage_type(),
                    value.layout().size().get(),
                    value.layout().alignment(),
                ))
                .expect("LIR Scoop ABI passing agrees with canonical storage shape")
            }
        };
        identity::CanonicalScoopAbiFunctionSignature::new(
            exact_signature,
            arguments,
            result,
            match gc_effect {
                mir::GcEffect::Managed => identity::GcEffect::Managed,
                mir::GcEffect::NoGc => identity::GcEffect::NoGc,
            },
        )
        .expect("the canonical Scoop ABI preserves the exact logical signature")
    }

    fn scoop_storage(
        &self,
        exact_type: identity::PersistentExactTypeId,
        ty: &lir::LirType,
        byte_size: u64,
        alignment: NonZeroU64,
    ) -> identity::CanonicalScoopStorage {
        let shape = lir::scoop_abi_value_shape(self.enums, ty)
            .expect("validated Scoop ABI storage has a non-void value shape");
        identity::CanonicalScoopStorage::new(
            exact_type,
            byte_size,
            alignment,
            match shape {
                lir::ScoopAbiValueShape::Scalar => identity::ScoopAbiValueShape::Scalar,
                lir::ScoopAbiValueShape::Aggregate => identity::ScoopAbiValueShape::Aggregate,
            },
        )
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

fn source_target(
    contract: &identity::SourceNativeExternalContract,
) -> (
    &identity::SourceNativeSymbol,
    &identity::SourceNativeLibraryBinding,
) {
    match contract {
        identity::SourceNativeExternalContract::Function {
            symbol, library, ..
        }
        | identity::SourceNativeExternalContract::ReadOnlyData {
            symbol, library, ..
        }
        | identity::SourceNativeExternalContract::MutableData {
            symbol, library, ..
        }
        | identity::SourceNativeExternalContract::ReadOnlyTls {
            symbol, library, ..
        }
        | identity::SourceNativeExternalContract::MutableTls {
            symbol, library, ..
        } => (symbol, library),
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
