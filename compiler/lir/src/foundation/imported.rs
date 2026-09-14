use std::fmt;

use scoop_identity::{
    ConeIdentity, ExactCallableSignature, ImportedIdentityId, ImportedIdentityMap,
    LirIdentityLayer, ObjectDefinitionPlanId, PersistentCallableBodyId, PersistentExportBindingId,
    PersistentId, PersistentSymbolRequest, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::WireEncode;

use super::{
    CanonicalLirFoundation, LirFoundationCounts, OdrFreeLirFoundation, ValidatedLirFoundation,
};

/// Session-local LIR identity. It cannot be interchanged with imported HIR or
/// MIR identities even when the persistent kind is the same.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedLirId<I: PersistentId>(ImportedIdentityId<I>);

impl<I: PersistentId> ImportedLirId<I> {
    pub const fn persistent(self) -> I {
        self.0.persistent()
    }

    pub const fn session_index(self) -> u32 {
        self.0.into_u32()
    }
}

/// The LIR identity foundation after atomic import into a semantic session.
pub struct ImportedLirFoundation {
    canonical: CanonicalLirFoundation,
    identities: ImportedIdentityMap<LirIdentityLayer>,
}

impl ImportedLirFoundation {
    #[doc(hidden)]
    pub fn from_validated(
        foundation: ValidatedLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    #[doc(hidden)]
    pub fn from_odr_free(
        foundation: OdrFreeLirFoundation,
        identities: ImportedIdentityMap<LirIdentityLayer>,
    ) -> Self {
        Self {
            canonical: foundation.into_canonical(),
            identities,
        }
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.identities.origin()
    }

    pub fn counts(&self) -> LirFoundationCounts {
        self.canonical.counts()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.len()
    }

    pub fn identity<I: PersistentId + 'static>(&self, id: I) -> Option<ImportedLirId<I>> {
        self.identities.get(id).map(ImportedLirId)
    }

    /// Projects a callable only after the exact body, symbol, and canonical
    /// definition plan have all been proven by this imported LIR foundation
    /// and its matching production surface.
    pub fn project_core_callable<'a>(
        &'a self,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        binding: PersistentExportBindingId,
        target: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedLirCallable<'a>, ImportedLirCallableProjectionError> {
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedLirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let (body, expected_symbol, required_definition) =
            crate::core_callable_link_contract(target)
                .map_err(ImportedLirCallableProjectionError::Contract)?;
        let body = self
            .identity(body)
            .ok_or(ImportedLirCallableProjectionError::MissingBody(body))?;
        let required_definition = self.identity(required_definition).ok_or(
            ImportedLirCallableProjectionError::MissingDefinition(required_definition),
        )?;
        let plan = definitions.plan(required_definition.persistent()).ok_or(
            ImportedLirCallableProjectionError::MissingDefinition(required_definition.persistent()),
        )?;
        if plan.owner() != StrongDefinitionEntity::callable_body(body.persistent())
            || plan.definition_role() != StrongDefinitionRole::CallableBody
            || plan.primary_symbol() != expected_symbol
        {
            return Err(ImportedLirCallableProjectionError::DefinitionMismatch(
                required_definition.persistent(),
            ));
        }
        Ok(SelectedImportedLirCallable {
            foundation: self,
            definitions,
            binding,
            target,
            signature,
            body,
            expected_symbol,
            required_definition,
        })
    }
}

impl WireEncode for ImportedLirFoundation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.canonical.encode(encoder)
    }
}

/// The LIR-side authority for one callable imported from trusted core.
///
/// It retains the selected source binding, exact source signature, imported
/// callable-body and definition-plan identities, and the only legal strong
/// symbol request. Physical ABI lowering may consume this value; it cannot
/// recreate the requirement from a symbol spelling or a bare persistent id.
#[derive(Clone)]
pub struct SelectedImportedLirCallable<'a> {
    foundation: &'a ImportedLirFoundation,
    definitions: &'a crate::StrongObjectSymbolSurfaceV1,
    binding: PersistentExportBindingId,
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    body: ImportedLirId<PersistentCallableBodyId>,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ImportedLirId<ObjectDefinitionPlanId>,
}

impl SelectedImportedLirCallable<'_> {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    pub const fn body(&self) -> ImportedLirId<PersistentCallableBodyId> {
        self.body
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ImportedLirId<ObjectDefinitionPlanId> {
        self.required_definition
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedLirFoundation,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation) && std::ptr::eq(self.definitions, definitions)
    }
}

impl fmt::Debug for SelectedImportedLirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedLirCallable")
            .field("binding", &self.binding)
            .field("target", &self.target)
            .field("signature", &self.signature)
            .field("body", &self.body)
            .field("expected_symbol", &self.expected_symbol)
            .field("required_definition", &self.required_definition)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ImportedLirCallableProjectionError {
    FoundationNotCore(ConeIdentity),
    Contract(crate::CoreExternalBuildError),
    MissingBody(PersistentCallableBodyId),
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}

impl fmt::Display for ImportedLirCallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot project imported LIR callable: {self:?}")
    }
}

impl std::error::Error for ImportedLirCallableProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, DeclarationScope,
        DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, Effect, ExactTypeKey,
        ExportBindingKey, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
        PackagePath, PendingIdentityValidation, PersistentExactTypeId, PersistentFunctionId,
        PersistentSymbolKey, PersistentSymbolRequestTable, RuntimeIdentityRecord,
        SemanticIdentitySession, SemanticOriginFingerprint, SourceDeclarationKey,
        SourceDeclarationSite, StrongDefinitionEntity, StrongDefinitionRole,
    };
    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::*;
    use crate::{OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1};

    #[test]
    fn selected_callable_keeps_imported_body_and_definition_authority() {
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
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        let target = StrongCallableDefinitionOwner::Function(function);
        let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(target)).unwrap();
        let definition = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(
                ConeIdentity::CORE,
                StrongDefinitionEntity::callable_body(body.id()),
                StrongDefinitionRole::CallableBody,
            )
            .unwrap(),
        )
        .unwrap();
        let mut canonical = super::super::CanonicalLirFoundation::empty();
        canonical.set_callable_bodies(vec![body.clone()]).unwrap();
        canonical
            .set_definition_plans(vec![definition.clone()])
            .unwrap();
        canonical
            .set_definition_atoms(vec![
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    definition.id(),
                    DefinitionAtomRole::Primary,
                    DefinitionAtomSubkey::Singleton,
                ))
                .unwrap(),
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    definition.id(),
                    DefinitionAtomRole::EhFrame,
                    DefinitionAtomSubkey::Singleton,
                ))
                .unwrap(),
            ])
            .unwrap();
        let expected_symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::CallableBody(body.id()),
            LinkageClass::ConeStrong,
        )
        .unwrap();
        canonical
            .set_symbol_requests(PersistentSymbolRequestTable::new(vec![expected_symbol]).unwrap());
        let strong_foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical.clone()).unwrap();
        let definitions =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&strong_foundation).unwrap();
        let foundation = imported_foundation(canonical.clone(), function);
        let other_foundation = imported_foundation(canonical, function);
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
        let binding = PersistentExportBindingId::from_key(&ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new("run").unwrap(),
            BindingTarget::function(&declaration).unwrap(),
        ))
        .unwrap();

        let selected = foundation
            .project_core_callable(&definitions, binding, target, signature.clone())
            .unwrap();

        assert!(selected.belongs_to(&foundation, &definitions));
        assert!(!selected.belongs_to(&other_foundation, &definitions));
        assert_eq!(selected.binding(), binding);
        assert_eq!(selected.target(), target);
        assert_eq!(selected.signature(), &signature);
        assert_eq!(selected.body().persistent(), body.id());
        assert_eq!(selected.expected_symbol(), expected_symbol);
        assert_eq!(selected.required_definition().persistent(), definition.id());
    }

    fn imported_foundation(
        canonical: super::super::CanonicalLirFoundation,
        function: PersistentFunctionId,
    ) -> ImportedLirFoundation {
        let decoded = decode_canonical::<super::super::DecodedLirFoundation>(
            &encode(&canonical).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending.register_authority(function).unwrap();
        decoded.register_identities(&mut pending).unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        let identities = pending.finish().unwrap();
        let mut session = SemanticIdentitySession::new();
        let (_, _, imported) = session
            .import(
                ConeIdentity::CORE,
                SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
                &identities,
            )
            .unwrap()
            .into_parts();
        ImportedLirFoundation {
            canonical,
            identities: imported,
        }
    }
}
