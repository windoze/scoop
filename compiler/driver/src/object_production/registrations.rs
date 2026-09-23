//! Registration-family checks and leaf fingerprint production.

use super::*;

/// Atomic proof that all six strong registration families match the same
/// provisional Scoop object set and digest graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectVerifiedObjectProductionV1 {
    pub(in crate::object_production) production: PlannedBuiltinObjectProductionV1,
    pub(in crate::object_production) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(in crate::object_production) safepoint_registrations:
        VerifiedStrongSafepointRegistrationSetV1,
    pub(in crate::object_production) callable_registrations:
        VerifiedStrongCallableRegistrationSetV1,
    pub(in crate::object_production) type_registrations: VerifiedStrongTypeRegistrationSetV1,
    pub(in crate::object_production) immortal_object_registrations:
        VerifiedStrongImmortalObjectRegistrationSetV1,
    pub(in crate::object_production) static_storage_registrations:
        VerifiedStrongStaticStorageRegistrationSetV1,
    pub(in crate::object_production) initialization_registrations:
        VerifiedStrongInitializationRegistrationSetV1,
}

impl RegistrationObjectVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn safepoint_registrations(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.safepoint_registrations
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
    }

    pub const fn type_registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.type_registrations
    }

    pub const fn immortal_object_registrations(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.immortal_object_registrations
    }

    pub const fn static_storage_registrations(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.static_storage_registrations
    }

    pub const fn initialization_registrations(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationSetV1 {
        &self.initialization_registrations
    }

    pub fn fingerprint_registration_object_leaves(
        self,
    ) -> Result<RegistrationObjectLeafFingerprintedProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
        } = self;
        let (
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        ) = {
            let candidates = production.scoop_lir_candidates();
            let safepoints =
                compute_strong_safepoint_fingerprints_v1(safepoint_registrations, &candidates)
                    .map_err(BuiltinObjectProductionError::SafepointFingerprints)?;
            let callable_registration_objects =
                compute_strong_callable_registration_object_fingerprints_v1(
                    callable_registrations,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::CallableRegistrationObjectFingerprints)?;
            let type_registration_objects =
                compute_strong_type_registration_object_fingerprints_v1(
                    type_registrations,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::TypeRegistrationObjectFingerprints)?;
            let immortal_object_registration_objects =
                compute_strong_immortal_object_registration_object_fingerprints_v1(
                    immortal_object_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::ImmortalObjectRegistrationObjectFingerprints,
                )?;
            let static_storage_registration_objects =
                compute_strong_static_storage_registration_object_fingerprints_v1(
                    static_storage_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::StaticStorageRegistrationObjectFingerprints,
                )?;
            let initialization_registration_objects =
                compute_strong_initialization_registration_object_fingerprints_v1(
                    initialization_registrations,
                    &candidates,
                )
                .map_err(
                    BuiltinObjectProductionError::InitializationRegistrationObjectFingerprints,
                )?;
            (
                safepoints,
                callable_registration_objects,
                type_registration_objects,
                immortal_object_registration_objects,
                static_storage_registration_objects,
                initialization_registration_objects,
            )
        };

        Ok(
            RegistrationObjectLeafFingerprintedProductionV1::from_verified_leaves(
                production,
                symbol_plan,
                safepoints,
                callable_registration_objects,
                type_registration_objects,
                immortal_object_registration_objects,
                static_storage_registration_objects,
                initialization_registration_objects,
            ),
        )
    }
}
