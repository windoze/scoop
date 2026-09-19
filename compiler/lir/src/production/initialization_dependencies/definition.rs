use scoop_identity::{
    DigestNodeId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentSymbolRequest,
};

use super::*;
use crate::StrongInitializationUnitRegistrationPlanSet;

/// One addressable definition already joined to its symbol and primary atom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationArtifactRefV2 {
    symbol: PersistentSymbolRequest,
    plan: ObjectDefinitionPlanId,
    primary: ObjectDefinitionAtomId,
}

impl StrongInitializationArtifactRefV2 {
    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }
    pub const fn plan(&self) -> ObjectDefinitionPlanId {
        self.plan
    }
    pub const fn primary(&self) -> ObjectDefinitionAtomId {
        self.primary
    }
}

/// A physical unit reference retains all three independent definitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitDefinitionRefV2 {
    provider: ConeIdentity,
    unit: PersistentInitializationUnitId,
    descriptor: StrongInitializationArtifactRefV2,
    cell: StrongInitializationArtifactRefV2,
    registration: StrongInitializationArtifactRefV2,
    registration_fingerprint: DigestNodeId,
}

impl StrongInitializationUnitDefinitionRefV2 {
    pub fn from_registrations<D>(
        plans: &StrongInitializationUnitRegistrationPlanSet<D>,
        unit: PersistentInitializationUnitId,
    ) -> Option<Self> {
        let index = plans
            .registrations()
            .binary_search_by_key(&unit, |plan| plan.semantic().unit())
            .ok()?;
        let plan = &plans.registrations()[index];
        Some(Self {
            provider: plans.producer(),
            unit,
            descriptor: StrongInitializationArtifactRefV2 {
                symbol: plan.descriptor_symbol(),
                plan: plan.descriptor_definition_plan(),
                primary: plan.descriptor_primary_atom(),
            },
            cell: StrongInitializationArtifactRefV2 {
                symbol: plan.cell_symbol(),
                plan: plan.cell_definition_plan(),
                primary: plan.cell_primary_atom(),
            },
            registration: StrongInitializationArtifactRefV2 {
                symbol: plan.registration_symbol(),
                plan: plan.registration_definition_plan(),
                primary: plan.registration_primary_atom(),
            },
            registration_fingerprint: plan.registration_fingerprint_node(),
        })
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }
    pub const fn descriptor(&self) -> &StrongInitializationArtifactRefV2 {
        &self.descriptor
    }
    pub const fn cell(&self) -> &StrongInitializationArtifactRefV2 {
        &self.cell
    }
    pub const fn registration(&self) -> &StrongInitializationArtifactRefV2 {
        &self.registration
    }
    pub const fn registration_fingerprint(&self) -> DigestNodeId {
        self.registration_fingerprint
    }
}
