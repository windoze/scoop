use std::fmt;

use scoop_identity::{
    CallableOwner, ConeIdentity, ExactCallableSignature, ImportedIdentityId, ImportedIdentityMap,
    MirIdentityLayer, PersistentExportBindingId, PersistentFunctionId, PersistentId,
    StrongCallableDefinitionOwner,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalMirFoundation, MirFoundationCounts, OdrFreeMirFoundation, ValidatedMirFoundation,
};

/// Session-local MIR identity. It cannot be interchanged with imported HIR or
/// LIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedMirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedMirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The MIR identity foundation after atomic import into a semantic session.
pub struct ImportedMirFoundation {
    canonical: CanonicalMirFoundation,
    identities: ImportedIdentityMap<MirIdentityLayer>,
}

impl ImportedMirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeMirFoundation,
        identities: ImportedIdentityMap<MirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> MirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedMirId<I>> {
        self.identities.get(id).map(ImportedMirId)
    }

    /// Projects a callable only after replaying the complete core MIR bridge
    /// relation against this exact imported foundation.
    pub fn project_core_callable<'a>(
        &'a self,
        production: &'a crate::CoreBootstrapBridgeSectionV1,
        binding: PersistentExportBindingId,
        definition: PersistentFunctionId,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedMirCallable<'a>, ImportedMirCallableProjectionError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedMirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let crate::CoreMirBridgeBranchV1::Core(core_bridge) = production.core_bridge() else {
            return Err(ImportedMirCallableProjectionError::MissingCoreBridge);
        };
        let bridge = core_bridge
            .callable_targets()
            .iter()
            .find(|bridge| bridge.binding() == binding)
            .ok_or(ImportedMirCallableProjectionError::MissingCallable(binding))?;
        let implementation = CallableOwner::Function(definition);
        if bridge.definition() != definition || bridge.implementation() != implementation {
            return Err(ImportedMirCallableProjectionError::CallableMismatch(
                binding,
            ));
        }
        let strong_bridge = production
            .strong_callable_bridges()
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if strong_bridge.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        let foundation_signature = self
            .canonical
            .callable_signatures()
            .iter()
            .find(|candidate| {
                candidate.subject() == crate::CallableSignatureSubject::Strong(implementation)
            })
            .ok_or(ImportedMirCallableProjectionError::MissingStrongSignature(
                definition,
            ))?;
        if foundation_signature.signature() != &signature {
            return Err(ImportedMirCallableProjectionError::StrongSignatureMismatch(
                definition,
            ));
        }
        Ok(SelectedImportedMirCallable {
            foundation: self,
            production,
            binding,
            definition,
            implementation: StrongCallableDefinitionOwner::Function(definition),
            signature,
        })
    }
}

impl WireEncode for ImportedMirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

/// A param-free callable selected from the trusted core HIR surface and
/// proven against that artifact's MIR bridge.
///
/// Private borrows retain the exact imported foundation and production
/// surface that minted the value. The next stage can therefore reject a
/// selection from another artifact even when all persistent ids and payloads
/// are byte-identical.
#[derive(Clone)]
pub struct SelectedImportedMirCallable<'a> {
    foundation: &'a ImportedMirFoundation,
    production: &'a crate::CoreBootstrapBridgeSectionV1,
    binding: PersistentExportBindingId,
    definition: PersistentFunctionId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
}

impl SelectedImportedMirCallable<'_> {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(&self) -> PersistentFunctionId {
        self.definition
    }

    pub const fn implementation(&self) -> StrongCallableDefinitionOwner {
        self.implementation
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedMirFoundation,
        production: &crate::CoreBootstrapBridgeSectionV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.production, production)
    }
}

impl fmt::Debug for SelectedImportedMirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedMirCallable")
            .field("binding", &self.binding)
            .field("definition", &self.definition)
            .field("implementation", &self.implementation)
            .field("signature", &self.signature)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedMirCallableProjectionError {
    FoundationNotCore(ConeIdentity),
    MissingCoreBridge,
    MissingCallable(PersistentExportBindingId),
    CallableMismatch(PersistentExportBindingId),
    MissingStrongSignature(PersistentFunctionId),
    StrongSignatureMismatch(PersistentFunctionId),
}

impl fmt::Display for ImportedMirCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project imported MIR callable: {self:?}")
    }
}

impl std::error::Error for ImportedMirCallableProjectionError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, Effect,
        ExactTypeKey, ExportBindingKey, PackagePath, PendingIdentityValidation,
        PersistentExactTypeId, SemanticIdentitySession, SemanticOriginFingerprint,
        SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;
    use crate::{
        CallableSignatureRecord, CallableSignatureSubject, CoreBootstrapBridgeSectionV1,
        CoreMirBridgeBranchV1, CoreMirBridgeV1, CoreMirCallableBridgeV1, EntryMirBridgeBranchV1,
        StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1,
    };

    #[test]
    fn selected_callable_derives_the_only_strong_implementation() {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let definition = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new("run").unwrap(),
            BindingTarget::function(&declaration).unwrap(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
        let implementation = CallableOwner::Function(definition);
        let mut canonical = CanonicalMirFoundation::empty();
        canonical
            .set_callable_signatures(vec![CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(implementation),
                signature.clone(),
            )])
            .unwrap();
        let foundation = imported_foundation(canonical.clone());
        let other_foundation = imported_foundation(canonical);
        let production = CoreBootstrapBridgeSectionV1::try_new(
            ConeIdentity::CORE,
            CoreMirBridgeBranchV1::Core(
                CoreMirBridgeV1::try_new(vec![
                    CoreMirCallableBridgeV1::new(binding, definition, implementation).unwrap(),
                ])
                .unwrap(),
            ),
            EntryMirBridgeBranchV1::Library,
            StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
                implementation,
                signature.clone(),
            )])
            .unwrap(),
        )
        .unwrap();

        let selected = foundation
            .project_core_callable(&production, binding, definition, signature.clone())
            .unwrap();

        assert!(selected.belongs_to(&foundation, &production));
        assert!(!selected.belongs_to(&other_foundation, &production));
        assert_eq!(selected.binding(), binding);
        assert_eq!(selected.definition(), definition);
        assert_eq!(
            selected.implementation(),
            StrongCallableDefinitionOwner::Function(definition)
        );
        assert_eq!(selected.signature(), &signature);
    }

    fn imported_foundation(canonical: CanonicalMirFoundation) -> ImportedMirFoundation {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        let identities = pending.finish().unwrap();
        let mut session = SemanticIdentitySession::new();
        let (_, imported, _) = session
            .import(
                ConeIdentity::CORE,
                SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
                &identities,
            )
            .unwrap()
            .into_parts();
        ImportedMirFoundation {
            canonical,
            identities: imported,
        }
    }
}
