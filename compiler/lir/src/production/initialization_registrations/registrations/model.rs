//! Complete records shared by initialization registration versions.

use super::*;

pub type StrongInitializationUnitRegistrationPlanV1 =
    StrongInitializationUnitRegistrationPlan<PersistentInitializationUnitId>;
pub type StrongInitializationUnitRegistrationPlanV2 =
    StrongInitializationUnitRegistrationPlan<crate::StrongInitializationDependencyRefV2>;
pub type StrongInitializationUnitRegistrationPlanSetV1 =
    StrongInitializationUnitRegistrationPlanSet<PersistentInitializationUnitId>;
pub type StrongInitializationUnitRegistrationPlanSetV2 =
    StrongInitializationUnitRegistrationPlanSet<crate::StrongInitializationDependencyRefV2>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationStaticStorageRefPlanV1 {
    pub(super) storage: PersistentStaticStorageId,
    pub(super) storage_symbol: PersistentSymbolRequest,
    pub(super) registration_symbol: PersistentSymbolRequest,
    pub(super) registration_definition_plan: ObjectDefinitionPlanId,
    pub(super) registration_primary_atom: ObjectDefinitionAtomId,
    pub(super) registration_fingerprint_node: DigestNodeId,
}

impl StrongInitializationStaticStorageRefPlanV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn storage_symbol(self) -> PersistentSymbolRequest {
        self.storage_symbol
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationCallableRefPlanV1 {
    pub(super) body: PersistentCallableBodyId,
    pub(super) entry_symbol: PersistentSymbolRequest,
    pub(super) registration_symbol: PersistentSymbolRequest,
    pub(super) body_definition_plan: ObjectDefinitionPlanId,
    pub(super) body_primary_atom: ObjectDefinitionAtomId,
    pub(super) body_definition_node: DigestNodeId,
    pub(super) registration_definition_plan: ObjectDefinitionPlanId,
    pub(super) registration_primary_atom: ObjectDefinitionAtomId,
    pub(super) registration_fingerprint_node: DigestNodeId,
}

impl StrongInitializationCallableRefPlanV1 {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn entry_symbol(self) -> PersistentSymbolRequest {
        self.entry_symbol
    }

    pub const fn registration_symbol(self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn body_definition_plan(self) -> ObjectDefinitionPlanId {
        self.body_definition_plan
    }

    pub const fn body_primary_atom(self) -> ObjectDefinitionAtomId {
        self.body_primary_atom
    }

    pub const fn body_definition_node(self) -> DigestNodeId {
        self.body_definition_node
    }

    pub const fn registration_definition_plan(self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn registration_fingerprint_node(self) -> DigestNodeId {
        self.registration_fingerprint_node
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationRegistrationSchedulePlanV1 {
    EagerStartup {
        gateway: Box<StrongInitializationCallableRefPlanV1>,
        gateway_definition_patch: DigestPatchIntentId,
    },
    LazyAccess,
}

impl StrongInitializationRegistrationSchedulePlanV1 {
    pub fn gateway(&self) -> Option<&StrongInitializationCallableRefPlanV1> {
        match self {
            Self::EagerStartup { gateway, .. } => Some(gateway),
            Self::LazyAccess => None,
        }
    }

    pub const fn gateway_definition_patch(&self) -> Option<DigestPatchIntentId> {
        match self {
            Self::EagerStartup {
                gateway_definition_patch,
                ..
            } => Some(*gateway_definition_patch),
            Self::LazyAccess => None,
        }
    }
}

/// Every typed definition, referenced registration, and digest writer needed
/// to emit one strong initialization-unit registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitRegistrationPlan<D> {
    pub(super) semantic: StrongInitializationUnitSemanticPlan<D>,
    pub(super) definition_owner: crate::RegistrationDefinitionOwner,
    pub(super) registration_symbol: PersistentSymbolRequest,
    pub(super) registration_definition_plan: ObjectDefinitionPlanId,
    pub(super) registration_primary_atom: ObjectDefinitionAtomId,
    pub(super) cell_symbol: PersistentSymbolRequest,
    pub(super) cell_definition_plan: ObjectDefinitionPlanId,
    pub(super) cell_primary_atom: ObjectDefinitionAtomId,
    pub(super) descriptor_symbol: PersistentSymbolRequest,
    pub(super) descriptor_definition_plan: ObjectDefinitionPlanId,
    pub(super) descriptor_primary_atom: ObjectDefinitionAtomId,
    pub(super) diagnostic_atom: ObjectDefinitionAtomId,
    pub(super) storage: StrongInitializationStaticStorageRefPlanV1,
    pub(super) failure_root: StrongInitializationStaticStorageRefPlanV1,
    pub(super) initializer: StrongInitializationCallableRefPlanV1,
    pub(super) ensure: StrongInitializationCallableRefPlanV1,
    pub(super) schedule: StrongInitializationRegistrationSchedulePlanV1,
    pub(super) registration_object_node: DigestNodeId,
    pub(super) cell_definition_node: DigestNodeId,
    pub(super) descriptor_definition_node: DigestNodeId,
    pub(super) registration_fingerprint_node: DigestNodeId,
    pub(super) registration_definition_patch: DigestPatchIntentId,
}

impl<D> StrongInitializationUnitRegistrationPlan<D> {
    pub const fn definition_owner(&self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

    pub const fn semantic(&self) -> &StrongInitializationUnitSemanticPlan<D> {
        &self.semantic
    }

    pub const fn registration_symbol(&self) -> PersistentSymbolRequest {
        self.registration_symbol
    }

    pub const fn registration_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.registration_definition_plan
    }

    pub const fn registration_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.registration_primary_atom
    }

    pub const fn cell_symbol(&self) -> PersistentSymbolRequest {
        self.cell_symbol
    }

    pub const fn cell_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.cell_definition_plan
    }

    pub const fn cell_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.cell_primary_atom
    }

    pub const fn descriptor_symbol(&self) -> PersistentSymbolRequest {
        self.descriptor_symbol
    }

    pub const fn descriptor_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.descriptor_definition_plan
    }

    pub const fn descriptor_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.descriptor_primary_atom
    }

    pub const fn diagnostic_atom(&self) -> ObjectDefinitionAtomId {
        self.diagnostic_atom
    }

    pub const fn storage(&self) -> StrongInitializationStaticStorageRefPlanV1 {
        self.storage
    }

    pub const fn failure_root(&self) -> StrongInitializationStaticStorageRefPlanV1 {
        self.failure_root
    }

    pub const fn initializer(&self) -> StrongInitializationCallableRefPlanV1 {
        self.initializer
    }

    pub const fn ensure(&self) -> StrongInitializationCallableRefPlanV1 {
        self.ensure
    }

    pub const fn schedule(&self) -> &StrongInitializationRegistrationSchedulePlanV1 {
        &self.schedule
    }

    pub const fn registration_object_node(&self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn cell_definition_node(&self) -> DigestNodeId {
        self.cell_definition_node
    }

    pub const fn descriptor_definition_node(&self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn registration_fingerprint_node(&self) -> DigestNodeId {
        self.registration_fingerprint_node
    }

    pub const fn registration_definition_patch(&self) -> DigestPatchIntentId {
        self.registration_definition_patch
    }
}

/// Proof that the complete final-LIR initialization-unit set has exactly one
/// strong registration production plan per unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitRegistrationPlanSet<D> {
    pub(super) producer: ConeIdentity,
    pub(super) registrations: Vec<StrongInitializationUnitRegistrationPlan<D>>,
}
