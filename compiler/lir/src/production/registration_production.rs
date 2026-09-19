//! Closed registration-production authority for the M23-3 strong profile.

use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::{
    ImmortalObjectTypeRegistrationRefV1, Module, OdrFreeLirFoundation, RefScan,
    StaticStorageRelocationTableArtifactV1, StrongCallableRegistrationPlanBuildError,
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
    StrongCallableRuntimeScanPlanError, StrongCallableRuntimeScanPlanSetV1,
    StrongDigestFinalizationPlanV1, StrongImmortalObjectRegistrationPlanBuildError,
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanV1,
    StrongImmortalObjectSemanticPlanBuildError, StrongImmortalObjectSemanticPlanSetV1,
    StrongInitializationCallableRefPlanV1, StrongInitializationRegistrationSchedulePlanV1,
    StrongInitializationStaticStorageRefPlanV1, StrongInitializationUnitRegistrationPlanBuildError,
    StrongInitializationUnitRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanV1,
    StrongInitializationUnitSemanticPlanBuildError, StrongInitializationUnitSemanticPlanSetV1,
    StrongRegistrationIdentityBuildError, StrongRegistrationIdentitySurfaceV1,
    StrongSafepointRegistrationPlanBuildError, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongSafepointSemanticPlanV1,
    StrongStaticStorageInitialArtifactPlanV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageRegistrationPlanBuildError, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanV1, StrongTypeDescriptorSemanticPlanBuildError,
    StrongTypeDescriptorSemanticPlanSetV1, StrongTypeRegistrationPlanBuildError,
    StrongTypeRegistrationPlanSetV1,
};

mod wire;
pub use wire::*;

mod validation;
pub use validation::*;

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
    pub fn empty(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let identities = StrongRegistrationIdentitySurfaceV1::from_foundation(foundation, digests)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let type_semantics = StrongTypeDescriptorSemanticPlanSetV1::from_artifact(
            foundation.producer(),
            target.wire_id(),
            Vec::new(),
        );
        let callable_runtime_scans =
            StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(foundation)
                .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;
        Self::from_semantics(
            target,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            StrongSafepointSemanticPlanSetV1::from_artifact(foundation.producer(), Vec::new()),
            StrongImmortalObjectSemanticPlanSetV1::from_artifact(foundation.producer(), Vec::new()),
            StrongInitializationUnitSemanticPlanSetV1::from_artifact(
                crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(
                    foundation.producer(),
                    Vec::new(),
                ),
                Vec::new(),
            ),
        )
    }

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
        let type_semantics = StrongTypeDescriptorSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::TypeSemantics)?;
        let immortal_semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::ImmortalSemantics)?;
        let initialization_semantics =
            StrongInitializationUnitSemanticPlanSetV1::from_module(module)
                .map_err(StrongRegistrationProductionBuildError::InitializationSemantics)?;
        let callable_runtime_scans = StrongCallableRuntimeScanPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;

        Self::from_semantics(
            module.meta.target_profile,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            safepoint_semantics,
            immortal_semantics,
            initialization_semantics,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_semantics(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        identities: StrongRegistrationIdentitySurfaceV1,
        callable_runtime_scans: StrongCallableRuntimeScanPlanSetV1,
        type_semantics: StrongTypeDescriptorSemanticPlanSetV1,
        safepoint_semantics: StrongSafepointSemanticPlanSetV1,
        immortal_semantics: StrongImmortalObjectSemanticPlanSetV1,
        initialization_semantics: StrongInitializationUnitSemanticPlanSetV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let safepoints = StrongSafepointRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &safepoint_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Safepoints)?;
        let callables = StrongCallableRegistrationPlanSetV1::new(
            foundation,
            &identities,
            callable_runtime_scans,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Callables)?;
        let types = StrongTypeRegistrationPlanSetV1::new(
            target,
            foundation,
            &identities,
            &type_semantics,
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

    pub fn safepoint_semantics(&self) -> StrongSafepointSemanticPlanSetV1 {
        StrongSafepointSemanticPlanSetV1::from_artifact(
            self.safepoints.producer(),
            self.safepoints
                .registrations()
                .iter()
                .map(|plan| {
                    StrongSafepointSemanticPlanV1::from_artifact(
                        plan.site(),
                        plan.safepoint(),
                        plan.owner(),
                        plan.role(),
                        plan.root_pair_count(),
                    )
                })
                .collect(),
        )
    }
}

mod encode;

#[derive(Debug)]
pub enum StrongRegistrationProductionBuildError {
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    Identities(StrongRegistrationIdentityBuildError),
    CallableRuntimeScans(StrongCallableRuntimeScanPlanError),
    SafepointSemantics(StrongSafepointSemanticPlanError),
    TypeSemantics(StrongTypeDescriptorSemanticPlanBuildError),
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
