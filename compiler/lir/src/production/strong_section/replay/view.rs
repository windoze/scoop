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

    pub fn safepoint_semantics(&self) -> crate::StrongSafepointSemanticPlanSetV1 {
        self.section.registration_production().safepoint_semantics()
    }

    /// Accounts for one copy of each registration plan without exposing an
    /// encoder or promoting the whole candidate registration production.
    pub fn charge_registration_plan_copies(
        &self,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<(), scoop_wire::WireError> {
        let path = scoop_wire::WirePath::root();
        let bytes = scoop_wire::cbor::encoded_length(self.section.registration_production())
            .map_err(|_| {
                scoop_wire::WireError::new(
                    scoop_wire::WireErrorKind::IntegerOutOfRange,
                    path.clone(),
                    None,
                )
            })?;
        meter.charge_owned_bytes(bytes.saturating_mul(4), &path)?;
        meter.charge_work(bytes.saturating_mul(4), &path)
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

    pub fn shape_support_plan(&self) -> &ParamFreeShapeSupportPlanSetV1 {
        self.section.shape_support_plan()
    }

    pub fn generated_bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        self.section.generated_bridge_plan()
    }

    pub fn initialization_cycle_abi(&self) -> Option<&CallableAbiRecordV1> {
        self.section.initialization_cycle_abi()
    }
}
