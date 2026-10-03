//! Direct access to the registration records in a complete production section.

use super::*;

impl ConeProductionSectionV2 {
    pub fn provider(&self) -> ConeIdentity {
        self.type_registrations().producer()
    }

    pub fn registration_identities(&self) -> &crate::RegistrationIdentitySurfaceV1 {
        self.registration_production().identities()
    }

    pub fn type_registrations(&self) -> &crate::StrongTypeRegistrationPlanSetV2 {
        self.registration_production().types()
    }

    pub fn callable_registrations(&self) -> &crate::StrongCallableRegistrationPlanSetV1 {
        self.registration_production().callables()
    }

    pub fn safepoint_registrations(&self) -> &crate::StrongSafepointRegistrationPlanSetV1 {
        self.registration_production().safepoints()
    }

    pub fn safepoint_semantics(&self) -> crate::StrongSafepointSemanticPlanSetV1 {
        self.registration_production().safepoint_semantics()
    }

    pub fn immortal_registrations(&self) -> &crate::StrongImmortalObjectRegistrationPlanSetV1 {
        self.registration_production().immortal_objects()
    }

    pub fn static_storage_registrations(&self) -> &crate::StrongStaticStorageRegistrationPlanSetV1 {
        self.registration_production().static_storages()
    }

    pub fn initialization_registrations(
        &self,
    ) -> &crate::StrongInitializationUnitRegistrationPlanSetV2 {
        self.registration_production().initialization_units()
    }
}
