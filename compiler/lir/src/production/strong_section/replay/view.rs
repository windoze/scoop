//! Read-only facts for the later layout/ABI and selected-closure join.

use super::*;

/// Complete physical section replay, without source/export/selection proof.
/// No public conversion or wire encoder can turn this into a published V2
/// production section before the remaining semantic relations are joined.
#[derive(Debug)]
pub struct ReplayedStrongProductionSectionV2 {
    pub(in crate::production) section: StrongProductionSectionV2,
}

impl ReplayedStrongProductionSectionV2 {
    pub fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        self.section.external_bridges()
    }

    pub fn canonical_definitions(&self) -> &StrongObjectSymbolSurfaceV1 {
        self.section.canonical_definitions()
    }

    pub fn object_definition_plans(&self) -> &StrongObjectDefinitionPlanSurfaceV1 {
        self.section.object_definition_plans()
    }

    pub fn digest_finalization_plan(&self) -> &StrongDigestFinalizationPlanV1 {
        self.section.digest_finalization_plan()
    }

    pub fn registration_identities(&self) -> &crate::StrongRegistrationIdentitySurfaceV1 {
        self.section.registration_production().identities()
    }

    pub fn type_registrations(&self) -> &crate::StrongTypeRegistrationPlanSetV2 {
        self.section.registration_production().types()
    }

    pub fn callable_registrations(&self) -> &crate::StrongCallableRegistrationPlanSetV1 {
        self.section.registration_production().callables()
    }

    pub fn safepoint_registrations(&self) -> &crate::StrongSafepointRegistrationPlanSetV1 {
        self.section.registration_production().safepoints()
    }

    pub fn immortal_registrations(&self) -> &crate::StrongImmortalObjectRegistrationPlanSetV1 {
        self.section.registration_production().immortal_objects()
    }

    pub fn static_storage_registrations(&self) -> &crate::StrongStaticStorageRegistrationPlanSetV1 {
        self.section.registration_production().static_storages()
    }

    pub fn initialization_registrations(
        &self,
    ) -> &crate::StrongInitializationUnitRegistrationPlanSetV2 {
        self.section
            .registration_production()
            .initialization_units()
    }

    pub fn image_plan(&self) -> &ConeImagePlanV1 {
        self.section.image_plan()
    }

    pub fn entry_plan(&self) -> &EntryProductionPlanV1 {
        self.section.entry_plan()
    }

    pub fn core_shape_support(&self) -> &CoreShapeSupportPlanV1 {
        self.section.core_shape_support()
    }

    pub fn generated_bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        self.section.generated_bridge_plan()
    }

    pub fn core_lir_bridge(&self) -> &CoreLirBridgeBranchV1 {
        self.section.core_lir_bridge()
    }
}
