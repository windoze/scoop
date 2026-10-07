use super::*;

impl StrongRegistrationProductionSurfaceV2 {
    /// Builds an empty V2 registration surface directly from V2 semantic
    /// constituents. This is used by producers whose final LIR has no local
    /// runtime registrations; it does not reinterpret a V1 wire surface.
    pub fn empty(
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,
        digests: &DigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let producer = foundation.producer();
        let identities = RegistrationIdentitySurfaceV1::from_foundation(foundation)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let callable_runtime_scans =
            StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(foundation)
                .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;
        Self::from_semantics(
            target,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            crate::StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
                producer,
                target.wire_id(),
                Vec::new(),
            ),
            StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            crate::StrongInitializationUnitSemanticPlanSetV2::from_artifact(
                crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(producer, Vec::new()),
                Vec::new(),
            ),
        )
    }

    /// Computes local runtime semantics once for digests and registrations.
    pub fn from_module(
        module: &Module,
        foundation: &ConeLirFoundation,
        entry_source: &crate::EntryProductionSourceV1,
        external_initialization_uses: &[crate::StrongExternalInitializationUseV2],
    ) -> Result<(DigestFinalizationPlanV1, Self), StrongRegistrationProductionBuildError> {
        if module.cone != foundation.producer() {
            return Err(StrongRegistrationProductionBuildError::ProducerMismatch {
                module: module.cone,
                foundation: foundation.producer(),
            });
        }
        let safepoint_semantics = StrongSafepointSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::SafepointSemantics)?;
        let type_semantics = crate::StrongTypeDescriptorSemanticPlanSetV2::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::TypeSemantics)?;
        let immortal_semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::ImmortalSemantics)?;
        let local_initialization =
            StrongInitializationUnitSemanticPlanSetV1::from_module(module)
                .map_err(StrongRegistrationProductionBuildError::InitializationSemantics)?;
        let digests = crate::project_digest_finalization_plan(
            foundation,
            entry_source,
            &safepoint_semantics,
            &type_semantics,
            &local_initialization,
        )
        .map_err(StrongRegistrationProductionBuildError::Digests)?;
        let identities = RegistrationIdentitySurfaceV1::from_foundation(foundation)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let initialization_semantics =
            crate::StrongInitializationUnitSemanticPlanSetV2::from_local_semantics(
                local_initialization,
                foundation,
                &identities,
                external_initialization_uses,
            )
            .map_err(StrongRegistrationProductionBuildError::InitializationSemanticsV2)?;
        let callable_runtime_scans = StrongCallableRuntimeScanPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;
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
