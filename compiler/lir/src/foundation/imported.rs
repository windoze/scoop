use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    ConeIdentity, CoreImportedCallableKind, ExactCallableSignature, ImportedIdentityId,
    ImportedIdentityMap, LirIdentityLayer, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentId, PersistentSymbolRequest, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
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

struct CheckedImportedCoreCallable {
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    abi_signature: scoop_identity::CanonicalScoopAbiFunctionSignature,
    calling_convention: crate::CallingConvention,
    root_plan: crate::CoreExternalCallableRootPlan,
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

    pub fn project_initialization_cycle_thrower<'a>(
        &'a self,
        core_bridge: &'a crate::CoreLirBridgeV1,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        target: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
    ) -> Result<SelectedImportedLirCallable<'a>, ImportedLirCallableProjectionError> {
        let kind = CoreImportedCallableKind::InitializationCycleThrower;
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedLirCallableProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        let bridge = core_bridge.initialization_cycle_thrower();
        if bridge.target() != target {
            return Err(ImportedLirCallableProjectionError::TargetMismatch(kind));
        }
        if bridge.abi_signature().signature() != &signature {
            return Err(ImportedLirCallableProjectionError::SignatureMismatch(kind));
        }
        let (_, expected_symbol, required_definition) = crate::core_callable_link_contract(target)
            .map_err(ImportedLirCallableProjectionError::Contract)?;
        if bridge.expected_symbol() != expected_symbol
            || bridge.required_definition() != required_definition
        {
            return Err(ImportedLirCallableProjectionError::BridgeContractMismatch(
                kind,
            ));
        }
        self.project_checked_callable(
            definitions,
            core_bridge,
            kind,
            CheckedImportedCoreCallable {
                target,
                signature,
                abi_signature: bridge.abi_signature().clone(),
                calling_convention: bridge.calling_convention(),
                root_plan: bridge.root_plan(),
            },
        )
    }

    fn project_checked_callable<'a>(
        &'a self,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        core_bridge: &'a crate::CoreLirBridgeV1,
        kind: CoreImportedCallableKind,
        contract: CheckedImportedCoreCallable,
    ) -> Result<SelectedImportedLirCallable<'a>, ImportedLirCallableProjectionError> {
        let (body, expected_symbol, required_definition) =
            crate::core_callable_link_contract(contract.target)
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
            core_bridge,
            kind,
            target: contract.target,
            signature: contract.signature,
            abi_signature: contract.abi_signature,
            calling_convention: contract.calling_convention,
            root_plan: contract.root_plan,
            body,
            expected_symbol,
            required_definition,
        })
    }

    /// Projects one core-owned TypeDescriptor only after its exact type and
    /// canonical strong definition have both been proven by this imported
    /// LIR world. Public-surface capability checks remain the responsibility
    /// of the artifact-level caller that supplies `target`.
    pub fn project_core_type_descriptor<'a>(
        &'a self,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        target: PersistentExactTypeId,
    ) -> Result<SelectedImportedLirTypeDescriptor<'a>, ImportedLirTypeDescriptorProjectionError>
    {
        if self.origin() != ConeIdentity::CORE {
            return Err(ImportedLirTypeDescriptorProjectionError::FoundationNotCore(
                self.origin(),
            ));
        }
        if self
            .canonical
            .materialized_exact_types
            .binary_search(&target)
            .is_err()
        {
            return Err(ImportedLirTypeDescriptorProjectionError::MissingExactType(
                target,
            ));
        }
        let (expected_symbol, required_definition) =
            crate::core_type_descriptor_link_contract(target)
                .map_err(ImportedLirTypeDescriptorProjectionError::Contract)?;
        let required_definition = self.identity(required_definition).ok_or(
            ImportedLirTypeDescriptorProjectionError::MissingDefinition(required_definition),
        )?;
        let plan = definitions.plan(required_definition.persistent()).ok_or(
            ImportedLirTypeDescriptorProjectionError::MissingDefinition(
                required_definition.persistent(),
            ),
        )?;
        if plan.owner() != StrongDefinitionEntity::exact_type(target)
            || plan.definition_role() != StrongDefinitionRole::TypeDescriptor
            || plan.primary_symbol() != expected_symbol
        {
            return Err(
                ImportedLirTypeDescriptorProjectionError::DefinitionMismatch(
                    required_definition.persistent(),
                ),
            );
        }
        Ok(SelectedImportedLirTypeDescriptor {
            foundation: self,
            definitions,
            target,
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
    kind: CoreImportedCallableKind,
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    abi_signature: scoop_identity::CanonicalScoopAbiFunctionSignature,
    calling_convention: crate::CallingConvention,
    root_plan: crate::CoreExternalCallableRootPlan,
    body: ImportedLirId<PersistentCallableBodyId>,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ImportedLirId<ObjectDefinitionPlanId>,
}

/// The runtime String TypeDescriptor authority imported from trusted core.
///
/// The retained imported LIR foundation proves the exact-type reference, and
/// the retained definition plan proves the only legal strong symbol request.
#[derive(Clone)]
pub struct SelectedImportedLirTypeDescriptor<'a> {
    foundation: &'a ImportedLirFoundation,
    definitions: &'a crate::StrongObjectSymbolSurfaceV1,
    target: PersistentExactTypeId,
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
    by_kind: BTreeMap<CoreImportedCallableKind, ImportedCoreLirCallableId>,
    callables: Vec<SelectedImportedLirCallable<'a>>,
    runtime_string: SelectedImportedLirTypeDescriptor<'a>,
}

impl<'a> SelectedImportedLirSet<'a> {
    #[doc(hidden)]
    pub fn try_new(
        foundation: &'a ImportedLirFoundation,
        definitions: &'a crate::StrongObjectSymbolSurfaceV1,
        core_bridge: &'a crate::CoreLirBridgeV1,
        runtime_string: SelectedImportedLirTypeDescriptor<'a>,
    ) -> Result<Self, ImportedLirSelectionError> {
        if !runtime_string.belongs_to(foundation, definitions) {
            return Err(ImportedLirSelectionError::ForeignRuntimeString);
        }
        Ok(Self {
            foundation,
            definitions,
            core_bridge,
            by_kind: BTreeMap::new(),
            callables: Vec::new(),
            runtime_string,
        })
    }

    #[doc(hidden)]
    pub fn insert(
        &mut self,
        selected: SelectedImportedLirCallable<'a>,
    ) -> Result<ImportedCoreLirCallableId, ImportedLirSelectionError> {
        if !selected.belongs_to(self.foundation, self.definitions, self.core_bridge) {
            return Err(ImportedLirSelectionError::ForeignSelection(selected.kind()));
        }
        if let Some(&id) = self.by_kind.get(&selected.kind()) {
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
                    selected.kind(),
                ));
            }
            return Ok(id);
        }
        let id = ImportedCoreLirCallableId(
            u32::try_from(self.callables.len())
                .expect("one LIR request cannot select more than u32::MAX core callables"),
        );
        self.by_kind.insert(selected.kind(), id);
        self.callables.push(selected);
        Ok(id)
    }

    pub fn callable(
        &self,
        id: ImportedCoreLirCallableId,
    ) -> Option<&SelectedImportedLirCallable<'a>> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for_kind(
        &self,
        kind: CoreImportedCallableKind,
    ) -> Option<ImportedCoreLirCallableId> {
        self.by_kind.get(&kind).copied()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    pub const fn runtime_string(&self) -> &SelectedImportedLirTypeDescriptor<'a> {
        &self.runtime_string
    }

    #[doc(hidden)]
    pub fn callable_selections(&self) -> impl Iterator<Item = &SelectedImportedLirCallable<'a>> {
        self.callables.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedLirSelectionError {
    ForeignRuntimeString,
    ForeignSelection(CoreImportedCallableKind),
    ConflictingSelection(CoreImportedCallableKind),
}

impl fmt::Display for ImportedLirSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignRuntimeString => formatter.write_str(
                "imported LIR runtime String belongs to another core artifact projection",
            ),
            Self::ForeignSelection(kind) => write!(
                formatter,
                "imported LIR callable {kind:?} belongs to another core artifact projection"
            ),
            Self::ConflictingSelection(kind) => write!(
                formatter,
                "imported LIR callable {kind:?} has conflicting selected definitions"
            ),
        }
    }
}

impl std::error::Error for ImportedLirSelectionError {}

impl SelectedImportedLirCallable<'_> {
    pub const fn kind(&self) -> CoreImportedCallableKind {
        self.kind
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

impl SelectedImportedLirTypeDescriptor<'_> {
    pub const fn target(&self) -> PersistentExactTypeId {
        self.target
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ImportedLirId<ObjectDefinitionPlanId> {
        self.required_definition
    }

    pub fn materialize(
        &self,
    ) -> Result<crate::CoreExternalTypeDescriptor, crate::CoreExternalBuildError> {
        crate::CoreExternalTypeDescriptor::new(self.target())
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

impl fmt::Debug for SelectedImportedLirTypeDescriptor<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedLirTypeDescriptor")
            .field("target", &self.target)
            .field("expected_symbol", &self.expected_symbol)
            .field("required_definition", &self.required_definition)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SelectedImportedLirCallable<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelectedImportedLirCallable")
            .field("kind", &self.kind)
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
    TargetMismatch(CoreImportedCallableKind),
    SignatureMismatch(CoreImportedCallableKind),
    BridgeContractMismatch(CoreImportedCallableKind),
    Contract(crate::CoreExternalBuildError),
    MissingBody(PersistentCallableBodyId),
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}

#[derive(Debug)]
pub enum ImportedLirTypeDescriptorProjectionError {
    FoundationNotCore(ConeIdentity),
    MissingExactType(PersistentExactTypeId),
    Contract(crate::CoreExternalBuildError),
    MissingDefinition(ObjectDefinitionPlanId),
    DefinitionMismatch(ObjectDefinitionPlanId),
}

impl fmt::Display for ImportedLirTypeDescriptorProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project imported LIR TypeDescriptor: {self:?}"
        )
    }
}

impl std::error::Error for ImportedLirTypeDescriptorProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
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
mod tests;
