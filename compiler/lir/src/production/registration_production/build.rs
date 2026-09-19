//! Join each versioned constituent to one foundation and digest graph.

use super::*;

impl<
    D: crate::StrongDescriptorReference,
    C: Clone,
    I: crate::StrongInitializationDependencyReference,
> StrongRegistrationProductionSurface<D, C, I>
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_semantics(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        identities: StrongRegistrationIdentitySurfaceV1,
        callable_runtime_scans: StrongCallableRuntimeScanPlanSetV1,
        type_semantics: crate::StrongTypeDescriptorSemanticPlanSet<D, C>,
        safepoint_semantics: StrongSafepointSemanticPlanSetV1,
        immortal_semantics: StrongImmortalObjectSemanticPlanSetV1,
        initialization_semantics: crate::StrongInitializationUnitSemanticPlanSet<I>,
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
        let types = crate::StrongTypeRegistrationPlanSet::new(
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
        let initialization_units = crate::StrongInitializationUnitRegistrationPlanSet::new(
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
}
