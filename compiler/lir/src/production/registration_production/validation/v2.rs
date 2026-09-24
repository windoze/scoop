//! Complete physical replay, before source and selected-closure commitment.

use super::*;
use crate::{
    DecodedStrongRegistrationProductionSurfaceV2, StrongInitializationDefinitionCatalogV2,
    StrongRegistrationProductionSurfaceV2, StrongTypeReferenceDefinitionsV2,
};
use scoop_wire::{BudgetMeter, WirePath, encode_canonical_temporary_with_meter};

mod budget;

/// All eight registration fields have been replayed against one foundation.
/// This carrier grants no export or selected-dependency authority. It cannot
/// be encoded or converted publicly into a committed production surface.
#[derive(Debug)]
pub struct ReplayedStrongRegistrationProductionV2 {
    pub(in crate::production) surface: StrongRegistrationProductionSurfaceV2,
}

impl ReplayedStrongRegistrationProductionV2 {
    pub fn identities(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        self.surface.identities()
    }

    pub fn safepoints(&self) -> &crate::StrongSafepointRegistrationPlanSetV1 {
        self.surface.safepoints()
    }

    pub fn callables(&self) -> &crate::StrongCallableRegistrationPlanSetV1 {
        self.surface.callables()
    }

    pub fn types(&self) -> &crate::StrongTypeRegistrationPlanSetV2 {
        self.surface.types()
    }

    pub fn immortal_objects(&self) -> &crate::StrongImmortalObjectRegistrationPlanSetV1 {
        self.surface.immortal_objects()
    }

    pub fn static_storages(&self) -> &crate::StrongStaticStorageRegistrationPlanSetV1 {
        self.surface.static_storages()
    }

    pub fn initialization_units(&self) -> &crate::StrongInitializationUnitRegistrationPlanSetV2 {
        self.surface.initialization_units()
    }
}

impl DecodedStrongRegistrationProductionSurfaceV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        external_bridges: &StrongExternalLirBridgeSurfaceV1,
        type_definitions: &StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &StrongInitializationDefinitionCatalogV2,
        meter: &mut BudgetMeter,
    ) -> Result<ReplayedStrongRegistrationProductionV2, StrongRegistrationProductionValidationError>
    {
        // Empty tables must not allow authorities from another consumer.
        if external_bridges.producer() != foundation.producer()
            || type_definitions.consumer() != foundation.producer()
            || initialization_definitions.producer() != foundation.producer()
        {
            return Err(StrongRegistrationProductionValidationError::ProducerMismatch);
        }
        budget::charge_replay(&self, foundation, digests, external_bridges, meter)?;
        let path = WirePath::root();
        let actual = encode_canonical_temporary_with_meter(&self, meter, &path)?;
        let identities = self
            .identities
            .validate(foundation, digests)
            .map_err(StrongRegistrationProductionValidationError::Identities)?;
        let initialization_definitions = initialization_definitions.with_local_foundation(
            foundation,
            &identities,
            digests,
            meter,
        )?;
        let safepoints = validate_safepoints(self.safepoints, foundation, &identities)?;
        let callable_runtime_scans =
            validate_callable_runtime_scans(self.callable_runtime_scans, foundation)?;
        let types = validate_type_registration_constituents_v2(
            self.types,
            target,
            foundation,
            &identities,
            external_bridges,
            type_definitions,
            digests,
            meter,
        )?;
        let immortal_objects = validate_immortal_objects(
            self.immortal_objects,
            target,
            foundation,
            &identities,
            external_bridges,
        )?;
        let storages =
            validate_static_storages(self.static_storages, target, foundation, &identities)?;
        let initialization = validate_initialization_registration_constituents_v2(
            self.initialization_units,
            target,
            foundation,
            &identities,
            storages,
            &initialization_definitions,
            digests,
            meter,
        )?;
        let surface = StrongRegistrationProductionSurfaceV2::from_semantics(
            target,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            types,
            safepoints,
            immortal_objects,
            initialization,
        )
        .map_err(|error| StrongRegistrationProductionValidationError::Expected(Box::new(error)))?;
        if actual != encode_canonical_temporary_with_meter(&surface, meter, &path)? {
            return Err(StrongRegistrationProductionValidationError::SurfaceMismatch);
        }
        Ok(ReplayedStrongRegistrationProductionV2 { surface })
    }
}
