//! Closed registration-production authority for the M23-3 strong profile.

use std::fmt;

use crate::{
    Module, OdrFreeLirFoundation, StrongCallableRegistrationPlanBuildError,
    StrongCallableRegistrationPlanSetV1, StrongDigestFinalizationPlanV1,
    StrongImmortalObjectRegistrationPlanBuildError, StrongImmortalObjectRegistrationPlanSetV1,
    StrongImmortalObjectSemanticPlanBuildError, StrongImmortalObjectSemanticPlanSetV1,
    StrongInitializationUnitRegistrationPlanBuildError,
    StrongInitializationUnitRegistrationPlanSetV1, StrongInitializationUnitSemanticPlanBuildError,
    StrongInitializationUnitSemanticPlanSetV1, StrongRegistrationIdentityBuildError,
    StrongRegistrationIdentitySurfaceV1, StrongSafepointRegistrationPlanBuildError,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongStaticStorageRegistrationPlanBuildError,
    StrongStaticStorageRegistrationPlanSetV1, StrongTypeRegistrationPlanBuildError,
    StrongTypeRegistrationPlanSetV1,
};

/// Complete member-independent registration authority produced from one final
/// LIR module. Every table has already been checked against the same foundation
/// and digest graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRegistrationProductionSurfaceV1 {
    identities: StrongRegistrationIdentitySurfaceV1,
    safepoints: StrongSafepointRegistrationPlanSetV1,
    callables: StrongCallableRegistrationPlanSetV1,
    types: StrongTypeRegistrationPlanSetV1,
    immortal_objects: StrongImmortalObjectRegistrationPlanSetV1,
    static_storages: StrongStaticStorageRegistrationPlanSetV1,
    initialization_units: StrongInitializationUnitRegistrationPlanSetV1,
}

impl StrongRegistrationProductionSurfaceV1 {
    pub fn from_module(
        module: &Module,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        if module.cone != foundation.producer() {
            return Err(StrongRegistrationProductionBuildError::ProducerMismatch {
                module: module.cone,
                foundation: foundation.producer(),
            });
        }

        let identities = StrongRegistrationIdentitySurfaceV1::from_foundation(foundation, digests)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let safepoint_semantics = StrongSafepointSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::SafepointSemantics)?;
        let immortal_semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::ImmortalSemantics)?;
        let initialization_semantics =
            StrongInitializationUnitSemanticPlanSetV1::from_module(module)
                .map_err(StrongRegistrationProductionBuildError::InitializationSemantics)?;

        let safepoints = StrongSafepointRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &safepoint_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Safepoints)?;
        let callables = StrongCallableRegistrationPlanSetV1::new(foundation, &identities, digests)
            .map_err(StrongRegistrationProductionBuildError::Callables)?;
        let types = StrongTypeRegistrationPlanSetV1::new(
            module.meta.target_profile,
            foundation,
            &identities,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Types)?;
        let immortal_objects = StrongImmortalObjectRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &immortal_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::ImmortalObjects)?;
        let static_storages = StrongStaticStorageRegistrationPlanSetV1::new(
            foundation,
            &identities,
            initialization_semantics.static_storages(),
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::StaticStorages)?;
        let initialization_units = StrongInitializationUnitRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &initialization_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::InitializationUnits)?;

        Ok(Self {
            identities,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initialization_units,
        })
    }

    pub const fn identities(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        &self.identities
    }

    pub const fn safepoints(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &StrongTypeRegistrationPlanSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        &self.static_storages
    }

    pub const fn initialization_units(&self) -> &StrongInitializationUnitRegistrationPlanSetV1 {
        &self.initialization_units
    }
}

#[derive(Debug)]
pub enum StrongRegistrationProductionBuildError {
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    Identities(StrongRegistrationIdentityBuildError),
    SafepointSemantics(StrongSafepointSemanticPlanError),
    ImmortalSemantics(StrongImmortalObjectSemanticPlanBuildError),
    InitializationSemantics(StrongInitializationUnitSemanticPlanBuildError),
    Safepoints(StrongSafepointRegistrationPlanBuildError),
    Callables(StrongCallableRegistrationPlanBuildError),
    Types(StrongTypeRegistrationPlanBuildError),
    ImmortalObjects(StrongImmortalObjectRegistrationPlanBuildError),
    StaticStorages(StrongStaticStorageRegistrationPlanBuildError),
    InitializationUnits(StrongInitializationUnitRegistrationPlanBuildError),
}

impl fmt::Display for StrongRegistrationProductionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong registration production surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationProductionBuildError {}
