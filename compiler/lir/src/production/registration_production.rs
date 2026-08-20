//! Closed registration-production authority for the M23-3 strong profile.

use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::{
    ConeLirFoundation, DigestFinalizationPlanV1, ImmortalObjectTypeRegistrationRefV1, Module,
    RefScan, RegistrationIdentityBuildError, RegistrationIdentitySurfaceV1,
    StaticStorageRelocationTableArtifactV1, StrongCallableRegistrationPlanBuildError,
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
    StrongCallableRuntimeScanPlanError, StrongCallableRuntimeScanPlanSetV1,
    StrongImmortalObjectRegistrationPlanBuildError, StrongImmortalObjectRegistrationPlanSetV1,
    StrongImmortalObjectRegistrationPlanV1, StrongImmortalObjectSemanticPlanBuildError,
    StrongImmortalObjectSemanticPlanSetV1, StrongInitializationCallableRefPlanV1,
    StrongInitializationRegistrationSchedulePlanV1, StrongInitializationStaticStorageRefPlanV1,
    StrongInitializationUnitRegistrationPlanBuildError,
    StrongInitializationUnitSemanticPlanBuildError, StrongInitializationUnitSemanticPlanSetV1,
    StrongSafepointRegistrationPlanBuildError, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongSafepointSemanticPlanV1,
    StrongStaticStorageInitialArtifactPlanV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageRegistrationPlanBuildError, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanV1, StrongTypeDescriptorSemanticPlanBuildError,
    StrongTypeDescriptorSemanticPlanSetV1, StrongTypeRegistrationPlanBuildError,
};

mod wire;
pub use wire::*;

mod validation;
pub use validation::*;

mod model;
pub use model::*;

mod v2;

impl StrongRegistrationProductionSurfaceV1 {
    pub fn empty(
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let identities = RegistrationIdentitySurfaceV1::from_foundation(foundation)
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
        foundation: &ConeLirFoundation,
        entry_source: &crate::EntryProductionSourceV1,
    ) -> Result<(DigestFinalizationPlanV1, Self), StrongRegistrationProductionBuildError> {
        if module.cone != foundation.producer() {
            return Err(StrongRegistrationProductionBuildError::ProducerMismatch {
                module: module.cone,
                foundation: foundation.producer(),
            });
        }

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

        let digests = crate::project_digest_finalization_plan(
            foundation,
            entry_source,
            &safepoint_semantics,
            &type_semantics,
            &initialization_semantics,
        )
        .map_err(StrongRegistrationProductionBuildError::Digests)?;
        let identities = RegistrationIdentitySurfaceV1::from_foundation(foundation)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let registrations = Self::from_semantics(
            module.meta.target_profile,
            foundation,
            &digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            safepoint_semantics,
            immortal_semantics,
            initialization_semantics,
        )?;
        Ok((digests, registrations))
    }
}

mod build;

mod encode;
pub use encode::{
    StrongInitializationUnitSemanticProjectionV1, StrongStaticStorageSemanticProjectionV1,
};

#[derive(Debug)]
pub enum StrongRegistrationProductionBuildError {
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    Digests(crate::DigestProjectionError),
    Identities(RegistrationIdentityBuildError),
    CallableRuntimeScans(StrongCallableRuntimeScanPlanError),
    SafepointSemantics(StrongSafepointSemanticPlanError),
    TypeSemantics(StrongTypeDescriptorSemanticPlanBuildError),
    ImmortalSemantics(StrongImmortalObjectSemanticPlanBuildError),
    InitializationSemantics(StrongInitializationUnitSemanticPlanBuildError),
    InitializationSemanticsV2(crate::StrongInitializationUnitSemanticPlanV2BuildError),
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
