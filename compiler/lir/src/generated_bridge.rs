//! Persistent identities for generated native bridge definitions.

use scoop_identity::{
    CallbackParameterIndex, CanonicalCAbiSignatureFingerprint, CborIdentityRecord, ConeIdentity,
    LinkageClass, PersistentSymbolKey, PersistentSymbolRequest, StaticNoGcCallbackStorageBridgeId,
};
pub use scoop_identity::{
    GeneratedBridgeAtomId, GeneratedBridgeAtomKey, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitId, GeneratedBridgeUnitKey,
};

use crate::MaterializedSymbol;

type BridgeUnitRecord = CborIdentityRecord<GeneratedBridgeUnitId, GeneratedBridgeUnitKey>;
type BridgeAtomRecord = CborIdentityRecord<GeneratedBridgeAtomId, GeneratedBridgeAtomKey>;

/// The producer-independent bridge recipe and producer-specific primary entry
/// that back one generated C definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedBridgeEntryIdentity {
    unit: BridgeUnitRecord,
    primary: MaterializedBridgeAtom,
}

impl GeneratedBridgeEntryIdentity {
    pub fn new(
        producer: ConeIdentity,
        key: GeneratedBridgeUnitKey,
    ) -> Result<Self, scoop_wire::HashError> {
        let unit = CborIdentityRecord::from_key(key)?;
        let primary = MaterializedBridgeAtom::new(
            producer,
            GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: unit.id() },
        )?;
        Ok(Self { unit, primary })
    }

    pub const fn unit(&self) -> GeneratedBridgeUnitId {
        self.unit.id()
    }

    pub const fn unit_record(&self) -> &BridgeUnitRecord {
        &self.unit
    }

    pub const fn primary_record(&self) -> &BridgeAtomRecord {
        self.primary.record()
    }

    pub const fn symbol_request(&self) -> PersistentSymbolRequest {
        self.primary.symbol_request()
    }

    pub fn symbol(&self) -> &str {
        self.primary.symbol()
    }
}

/// A static NoGC callback's generated-C trampoline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticCallbackTrampolineIdentity {
    entry: GeneratedBridgeEntryIdentity,
    storage_bridge: StaticNoGcCallbackStorageBridgeId,
    signature: CanonicalCAbiSignatureFingerprint,
}

impl StaticCallbackTrampolineIdentity {
    pub fn new(
        producer: ConeIdentity,
        storage_bridge: StaticNoGcCallbackStorageBridgeId,
        signature: CanonicalCAbiSignatureFingerprint,
    ) -> Result<Self, scoop_wire::HashError> {
        let entry = GeneratedBridgeEntryIdentity::new(
            producer,
            GeneratedBridgeUnitKey::StaticCallbackTrampoline {
                storage_bridge,
                signature,
            },
        )?;
        Ok(Self {
            entry,
            storage_bridge,
            signature,
        })
    }

    pub const fn entry(&self) -> &GeneratedBridgeEntryIdentity {
        &self.entry
    }

    pub const fn storage_bridge(&self) -> StaticNoGcCallbackStorageBridgeId {
        self.storage_bridge
    }

    pub const fn signature(&self) -> CanonicalCAbiSignatureFingerprint {
        self.signature
    }
}

/// A managed-callback C trampoline and its link-visible signature descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedCallbackTrampolineIdentity {
    entry: GeneratedBridgeEntryIdentity,
    signature_descriptor: MaterializedBridgeAtom,
    signature: CanonicalCAbiSignatureFingerprint,
}

impl ManagedCallbackTrampolineIdentity {
    pub fn new(
        producer: ConeIdentity,
        signature: CanonicalCAbiSignatureFingerprint,
        context_index: CallbackParameterIndex,
    ) -> Result<Self, scoop_wire::HashError> {
        let entry = GeneratedBridgeEntryIdentity::new(
            producer,
            GeneratedBridgeUnitKey::CallbackTrampoline {
                signature,
                context_index,
            },
        )?;
        let signature_descriptor = MaterializedBridgeAtom::new(
            producer,
            GeneratedBridgeAtomRoleKey::SignatureDescriptor {
                unit: entry.unit(),
                signature,
            },
        )?;
        Ok(Self {
            entry,
            signature_descriptor,
            signature,
        })
    }

    pub const fn entry(&self) -> &GeneratedBridgeEntryIdentity {
        &self.entry
    }

    pub const fn signature(&self) -> CanonicalCAbiSignatureFingerprint {
        self.signature
    }

    pub const fn signature_descriptor_record(&self) -> &BridgeAtomRecord {
        self.signature_descriptor.record()
    }

    pub const fn signature_descriptor_symbol_request(&self) -> PersistentSymbolRequest {
        self.signature_descriptor.symbol_request()
    }

    pub fn signature_descriptor_symbol(&self) -> &str {
        self.signature_descriptor.symbol()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MaterializedBridgeAtom {
    record: BridgeAtomRecord,
    symbol: MaterializedSymbol,
}

impl MaterializedBridgeAtom {
    fn new(
        producer: ConeIdentity,
        role: GeneratedBridgeAtomRoleKey,
    ) -> Result<Self, scoop_wire::HashError> {
        let record = CborIdentityRecord::from_key(GeneratedBridgeAtomKey::new(producer, role))?;
        let symbol = MaterializedSymbol::new(
            PersistentSymbolKey::GeneratedBridge(record.id()),
            LinkageClass::ConeStrong,
        )
        .expect("generated bridge atoms admit only their Cone-strong symbol class");
        Ok(Self { record, symbol })
    }

    const fn record(&self) -> &BridgeAtomRecord {
        &self.record
    }

    const fn symbol_request(&self) -> PersistentSymbolRequest {
        self.symbol.request()
    }

    fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CanonicalCAbiFunctionSignature, CanonicalCAbiReturn,
        CanonicalCAbiSignatureFingerprintRecord, PersistentSymbolKey,
    };

    use super::*;

    fn signature() -> CanonicalCAbiSignatureFingerprint {
        CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
            Vec::new(),
            CanonicalCAbiReturn::Void,
        ))
        .unwrap()
        .fingerprint()
    }

    #[test]
    fn static_callback_symbols_keep_their_stable_storage_bridge_target() {
        let signature = signature();
        let storage_bridge = static_storage_bridge(3);
        let first = StaticCallbackTrampolineIdentity::new(
            ConeIdentity::SINGLE_FILE,
            storage_bridge,
            signature,
        )
        .unwrap();
        let other_target = StaticCallbackTrampolineIdentity::new(
            ConeIdentity::SINGLE_FILE,
            static_storage_bridge(4),
            signature,
        )
        .unwrap();
        let other_producer =
            StaticCallbackTrampolineIdentity::new(ConeIdentity::CORE, storage_bridge, signature)
                .unwrap();

        assert_eq!(first.storage_bridge(), storage_bridge);
        assert_eq!(first.signature(), signature);
        assert_ne!(first.entry().unit(), other_target.entry().unit());
        assert_eq!(first.entry().unit(), other_producer.entry().unit());
        assert_ne!(
            first.entry().primary_record().id(),
            other_producer.entry().primary_record().id()
        );
    }

    fn static_storage_bridge(seed: u8) -> StaticNoGcCallbackStorageBridgeId {
        use scoop_identity::{
            CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
            CanonicalIdentifier, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain,
            Effect, ExactCallableSignature, ExactTypeKey, GeneratedCallableKey, PackagePath,
            PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
        };

        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(&format!("callback{seed}")).unwrap(),
            0,
            None,
            Vec::new(),
        );
        let source = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        let unit = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
        .id();

        StaticNoGcCallbackStorageBridgeId::from_key(
            &GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
                source: CallableMaterialization::new(
                    CallableTemplateOwner::Function(source),
                    CallableMaterializationContext::NoSubstitution,
                ),
                signature: ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
            },
        )
        .unwrap()
    }

    #[test]
    fn callback_symbols_are_typed_atoms_of_one_shared_recipe() {
        let signature = signature();
        let first = ManagedCallbackTrampolineIdentity::new(
            ConeIdentity::SINGLE_FILE,
            signature,
            CallbackParameterIndex::new(0),
        )
        .unwrap();
        let repeated = ManagedCallbackTrampolineIdentity::new(
            ConeIdentity::SINGLE_FILE,
            signature,
            CallbackParameterIndex::new(0),
        )
        .unwrap();
        let other_producer = ManagedCallbackTrampolineIdentity::new(
            ConeIdentity::CORE,
            signature,
            CallbackParameterIndex::new(0),
        )
        .unwrap();

        assert_eq!(first, repeated);
        assert_eq!(first.entry().unit(), other_producer.entry().unit());
        assert_ne!(
            first.entry().primary_record().id(),
            other_producer.entry().primary_record().id()
        );
        assert_ne!(first.entry().symbol(), first.signature_descriptor_symbol());
        assert_eq!(
            first.entry().symbol_request().key(),
            PersistentSymbolKey::GeneratedBridge(first.entry().primary_record().id())
        );
        assert_eq!(
            first.entry().symbol_request().linkage(),
            LinkageClass::ConeStrong
        );
    }
}
