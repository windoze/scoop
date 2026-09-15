use std::collections::BTreeMap;
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
        core_bridge: &'a crate::CoreLirBridgeV1,
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
        let bridge = core_bridge
            .callable(binding)
            .ok_or(ImportedLirCallableProjectionError::MissingBinding(binding))?;
        if bridge.target() != target {
            return Err(ImportedLirCallableProjectionError::TargetMismatch(binding));
        }
        if bridge.abi_signature().signature() != &signature {
            return Err(ImportedLirCallableProjectionError::SignatureMismatch(
                binding,
            ));
        }
        let (body, expected_symbol, required_definition) =
            crate::core_callable_link_contract(target)
                .map_err(ImportedLirCallableProjectionError::Contract)?;
        if bridge.expected_symbol() != expected_symbol
            || bridge.required_definition() != required_definition
        {
            return Err(ImportedLirCallableProjectionError::BridgeContractMismatch(
                binding,
            ));
        }
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
            core_bridge,
            binding,
            target,
            signature,
            abi_signature: bridge.abi_signature().clone(),
            calling_convention: bridge.calling_convention(),
            root_plan: bridge.root_plan(),
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
    core_bridge: &'a crate::CoreLirBridgeV1,
    binding: PersistentExportBindingId,
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    abi_signature: scoop_identity::CanonicalScoopAbiFunctionSignature,
    calling_convention: crate::CallingConvention,
    root_plan: crate::CoreExternalCallableRootPlan,
    body: ImportedLirId<PersistentCallableBodyId>,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ImportedLirId<ObjectDefinitionPlanId>,
}

/// Request-local LIR id for one callable selected from trusted core.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImportedCoreLirCallableId(u32);

/// Complete LIR external-callable authority for one ordinary lowering.
///
/// The set is inseparably bound to one imported LIR foundation and its
/// strong-definition surface. It retains the exact body, definition plan,
/// and symbol request proven for every selected callable.
pub struct SelectedImportedLirSet<'a> {
    foundation: &'a ImportedLirFoundation,
    definitions: &'a crate::StrongObjectSymbolSurfaceV1,
    core_bridge: &'a crate::CoreLirBridgeV1,
    by_binding: BTreeMap<PersistentExportBindingId, ImportedCoreLirCallableId>,
    callables: Vec<SelectedImportedLirCallable<'a>>,
}

impl<'a> SelectedImportedLirSet<'a> {
    #[doc(hidden)]
    pub fn new(
        foundation: &'a ImportedLirFoundation,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        core_bridge: &'a crate::CoreLirBridgeV1,
    ) -> Self {
        Self {
            foundation,
            definitions,
            core_bridge,
            by_binding: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn insert(
        &mut self,
        selected: SelectedImportedLirCallable<'a>,
    ) -> Result<ImportedCoreLirCallableId, ImportedLirSelectionError> {
        if !selected.belongs_to(self.foundation, self.definitions, self.core_bridge) {
            return Err(ImportedLirSelectionError::ForeignSelection(
                selected.binding(),
            ));
        }
        if let Some(&id) = self.by_binding.get(&selected.binding()) {
            let retained = &self.callables[id.0 as usize];
            if retained.target() != selected.target()
                || retained.signature() != selected.signature()
                || retained.abi_signature() != selected.abi_signature()
                || retained.calling_convention() != selected.calling_convention()
                || retained.root_plan() != selected.root_plan()
                || retained.body() != selected.body()
                || retained.expected_symbol() != selected.expected_symbol()
                || retained.required_definition() != selected.required_definition()
            {
                return Err(ImportedLirSelectionError::ConflictingSelection(
                    selected.binding(),
                ));
            }
            return Ok(id);
        }
        let id = ImportedCoreLirCallableId(
            u32::try_from(self.callables.len())
                .expect("one LIR request cannot select more than u32::MAX core callables"),
        );
        self.by_binding.insert(selected.binding(), id);
        self.callables.push(selected);
        Ok(id)
    }

    pub fn callable(
        &self,
        id: ImportedCoreLirCallableId,
    ) -> Option<&SelectedImportedLirCallable<'a>> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for_binding(
        &self,
        binding: PersistentExportBindingId,
    ) -> Option<ImportedCoreLirCallableId> {
        self.by_binding.get(&binding).copied()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    #[doc(hidden)]
    pub fn callable_selections(&self) -> impl Iterator<Item = &SelectedImportedLirCallable<'a>> {
        self.callables.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedLirSelectionError {
    ForeignSelection(PersistentExportBindingId),
    ConflictingSelection(PersistentExportBindingId),
}

impl fmt::Display for ImportedLirSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSelection(binding) => write!(
                formatter,
                "imported LIR binding {binding} belongs to another core artifact projection"
            ),
            Self::ConflictingSelection(binding) => write!(
                formatter,
                "imported LIR binding {binding} has conflicting selected definitions"
            ),
        }
    }
}

impl std::error::Error for ImportedLirSelectionError {}

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

    pub const fn abi_signature(&self) -> &scoop_identity::CanonicalScoopAbiFunctionSignature {
        &self.abi_signature
    }

    pub const fn calling_convention(&self) -> crate::CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> crate::CoreExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ImportedLirId<ObjectDefinitionPlanId> {
        self.required_definition
    }

    /// Materializes the transient LIR declaration only from this exact
    /// imported authority. The constructor itself is crate-private so a
    /// caller cannot fabricate a core external from persistent ids.
    pub fn materialize(
        &self,
        signature: crate::ScoopAbiSignature,
    ) -> Result<crate::CoreExternalCallable, crate::CoreExternalBuildError> {
        crate::CoreExternalCallable::new(
            self.target,
            self.abi_signature.clone(),
            signature,
            self.root_plan,
        )
    }

    #[doc(hidden)]
    pub fn belongs_to(
        &self,
        foundation: &ImportedLirFoundation,
        definitions: &crate::StrongObjectSymbolSurfaceV1,
        core_bridge: &crate::CoreLirBridgeV1,
    ) -> bool {
        std::ptr::eq(self.foundation, foundation)
            && std::ptr::eq(self.definitions, definitions)
            && std::ptr::eq(self.core_bridge, core_bridge)
    }
}

impl fmt::Debug for SelectedImportedLirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedLirCallable")
            .field("binding", &self.binding)
            .field("target", &self.target)
            .field("signature", &self.signature)
            .field("abi_signature", &self.abi_signature)
            .field("calling_convention", &self.calling_convention)
            .field("root_plan", &self.root_plan)
            .field("body", &self.body)
            .field("expected_symbol", &self.expected_symbol)
            .field("required_definition", &self.required_definition)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ImportedLirCallableProjectionError {
    FoundationNotCore(ConeIdentity),
    MissingBinding(PersistentExportBindingId),
    TargetMismatch(PersistentExportBindingId),
    SignatureMismatch(PersistentExportBindingId),
    BridgeContractMismatch(PersistentExportBindingId),
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
        BindingTarget, CallableBodyKey, CanonicalIdentifier, CanonicalScoopAbiFunctionSignature,
        CborIdentityRecord, DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey,
        DefinitionOwnerChain, Effect, ExactTypeKey, ExportBindingKey,
        GcEffect as CanonicalGcEffect, LinkageClass, ObjectDefinitionAtomKey,
        ObjectDefinitionPlanKey, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
        PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequestTable,
        RuntimeIdentityRecord, ScoopAbiReturn, SemanticIdentitySession, SemanticOriginFingerprint,
        SourceDeclarationKey, SourceDeclarationSite, StrongDefinitionEntity, StrongDefinitionRole,
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
        let abi_signature = CanonicalScoopAbiFunctionSignature::new(
            signature.clone(),
            Vec::new(),
            ScoopAbiReturn::unit_void(),
            CanonicalGcEffect::NoGc,
        )
        .unwrap();
        let core_bridge = crate::CoreLirBridgeV1::try_new(vec![
            crate::CoreLirCallableBridgeV1::new(
                binding,
                target,
                abi_signature,
                crate::CallingConvention::Cdecl,
                crate::CoreExternalCallableRootPlan::NoGc,
            )
            .unwrap(),
        ])
        .unwrap();

        let selected = foundation
            .project_core_callable(
                &core_bridge,
                &definitions,
                binding,
                target,
                signature.clone(),
            )
            .unwrap();
        let foreign = other_foundation
            .project_core_callable(
                &core_bridge,
                &definitions,
                binding,
                target,
                signature.clone(),
            )
            .unwrap();

        assert!(selected.belongs_to(&foundation, &definitions, &core_bridge));
        assert!(!selected.belongs_to(&other_foundation, &definitions, &core_bridge));
        assert_eq!(selected.binding(), binding);
        assert_eq!(selected.target(), target);
        assert_eq!(selected.signature(), &signature);
        assert_eq!(selected.body().persistent(), body.id());
        assert_eq!(selected.expected_symbol(), expected_symbol);
        assert_eq!(selected.required_definition().persistent(), definition.id());

        let mut selected_set = SelectedImportedLirSet::new(&foundation, &definitions, &core_bridge);
        let id = selected_set.insert(selected.clone()).unwrap();
        assert_eq!(selected_set.insert(selected).unwrap(), id);
        assert_eq!(selected_set.callable_for_binding(binding), Some(id));
        assert_eq!(selected_set.callable(id).unwrap().binding(), binding);
        assert_eq!(selected_set.len(), 1);
        assert_eq!(
            selected_set.insert(foreign).unwrap_err(),
            ImportedLirSelectionError::ForeignSelection(binding)
        );
        assert_eq!(selected_set.len(), 1);
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
