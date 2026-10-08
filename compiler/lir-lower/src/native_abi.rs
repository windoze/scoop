use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use scoop_identity as identity;

use super::*;

mod c_storage;
mod scoop;

pub(super) use scoop::canonical_scoop_signature;

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
pub(super) fn lower<'root>(
    context: &LoweringContext,
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    materialized_types: impl IntoIterator<Item = &'root mir::Type>,
) -> StorageResult<LoweredNativeAbi> {
    let mut builder = CanonicalCAbiBuilder::new(context.target_profile(), module, structs, enums);

    // An exported C-layout representation needs its complete canonical
    // contract even when no native callable mentions it.
    for ty in materialized_types {
        if let mir::Type::Struct(id) = ty
            && matches!(
                module.structs[*id].representation,
                mir::StructRepresentation::Declared {
                    c_layout: Some(_),
                    ..
                }
            )
        {
            builder.storage(ty)?;
        }
    }

    for (_, external) in module.extern_functions.iter() {
        builder.add_external_function(context, external)?;
    }
    for (_, global) in module.globals.iter() {
        if let mir::GlobalStorage::Extern {
            source_contract,
            thread_local,
            ..
        } = &global.storage
        {
            builder.add_external_global(source_contract, global, *thread_local)?;
        }
    }
    for (_, callback) in module.callback_bridges.iter() {
        builder.add_callback_signature(callback.signature)?;
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        builder.add_callback_signature(callback.native_signature)?;
    }

    Ok(builder.finish())
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

    fn add_external_function(
        &mut self,
        context: &LoweringContext,
        external: &mir::ExternFunction,
    ) -> StorageResult<()> {
        let (symbol, source_library) = source_target(external.source_contract.contract());
        let symbol_key = self.target_symbol(symbol);
        let library = self.library_binding(source_library);
        let contract = match external.abi {
            mir::ExternAbi::C(_) => identity::NativeExternalContract::c_function(
                library,
                self.add_signature(&external.params, external.result.native_type())?,
            ),
            mir::ExternAbi::Scoop => {
                let physical = abi::classify_mir_signature(
                    context,
                    self.module,
                    external.params.iter(),
                    external.result.native_type(),
                    self.structs,
                    self.enums,
                )?;
                identity::NativeExternalContract::scoop_function(
                    library,
                    canonical_scoop_signature(
                        self.module,
                        self.enums,
                        identity::ExactCallableSignature::new(
                            identity::Effect::Ordinary,
                            None,
                            external
                                .params
                                .iter()
                                .map(|ty| self.exact_type(ty))
                                .collect(),
                            self.exact_type(external.result.native_type()),
                        ),
                        &external.params,
                        external.result.native_type(),
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
        Ok(())
    }

    fn add_external_global(
        &mut self,
        source_contract: &identity::SourceNativeExternalContractRecord,
        global: &mir::Global,
        thread_local: bool,
    ) -> StorageResult<()> {
        let (symbol, source_library) = source_target(source_contract.contract());
        let symbol_key = self.target_symbol(symbol);
        let library = self.library_binding(source_library);
        let storage = self.storage(&global.ty)?;
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
        Ok(())
    }

    fn target_symbol(
        &self,
        symbol: &identity::SourceNativeSymbol,
    ) -> identity::NativeExternalSymbolKey {
        identity::NativeExternalSymbolKey::for_target(self.target_profile.wire_id(), symbol)
            .expect("validated source native symbols normalize for the selected target")
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
                    identity::NativeLinkRequirementKey::for_target(
                        self.target_profile.wire_id(),
                        name.clone(),
                        identity::NativeLibraryKind::TargetDefault,
                        identity::NativeLibraryGrouping::Independent,
                    ),
                )
                .expect("validated native library names have encodable requirements");
                self.link_requirements.push(record.clone());
                identity::NativeLibraryBinding::Requirement(record.id())
            }
        }
    }

    fn add_callback_signature(&mut self, id: mir::FunctionTypeId) -> StorageResult<()> {
        let signature = &self.module.function_types[id];
        let record = self.signature_record(&signature.parameter_types, &signature.return_type)?;
        let fingerprint = record.fingerprint();
        self.signatures.push(record);
        let previous = self.callback_signatures.insert(id, fingerprint);
        assert!(
            previous.is_none_or(|previous| previous == fingerprint),
            "one MIR function type has one canonical C ABI signature"
        );
        Ok(())
    }

    fn add_signature(
        &mut self,
        parameter_types: &[mir::Type],
        return_type: &mir::Type,
    ) -> StorageResult<identity::CanonicalCAbiFunctionSignature> {
        let record = self.signature_record(parameter_types, return_type)?;
        let signature = record.signature().clone();
        self.signatures.push(record);
        Ok(signature)
    }

    fn signature_record(
        &mut self,
        parameter_types: &[mir::Type],
        return_type: &mir::Type,
    ) -> StorageResult<identity::CanonicalCAbiSignatureFingerprintRecord> {
        let parameters = parameter_types
            .iter()
            .map(|ty| {
                let storage = self.storage(ty)?;
                Ok(
                    identity::CanonicalCAbiParameter::new(storage.exact_type(), storage)
                        .expect("canonical parameter storage retains its source exact type"),
                )
            })
            .collect::<StorageResult<Vec<_>>>()?;
        let result = if return_type == &mir::Type::Unit {
            identity::CanonicalCAbiReturn::Void
        } else {
            let storage = self.storage(return_type)?;
            identity::CanonicalCAbiReturn::value(storage.exact_type(), storage)
                .expect("canonical return storage retains its source exact type")
        };
        Ok(identity::CanonicalCAbiSignatureFingerprintRecord::new(
            identity::CanonicalCAbiFunctionSignature::cdecl(parameters, result),
        )
        .expect("canonical C ABI signatures have encodable identities"))
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
