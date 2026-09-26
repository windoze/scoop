//! Complete physical replay, before source and selected-closure commitment.

use super::*;
use crate::{
    DecodedStrongRegistrationProductionSurfaceV2, StrongInitializationDefinitionCatalogV2,
    StrongRegistrationProductionSurfaceV2, StrongTypeReferenceDefinitionsV2,
};
use scoop_wire::{WirePath, encode_canonical_temporary};

impl DecodedStrongRegistrationProductionSurfaceV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,

        type_definitions: &StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &StrongInitializationDefinitionCatalogV2,
    ) -> Result<StrongRegistrationProductionSurfaceV2, StrongRegistrationProductionValidationError>
    {
        // Every dependency directory must belong to this consumer.
        if type_definitions.consumer() != foundation.producer()
            || initialization_definitions.producer() != foundation.producer()
        {
            return Err(StrongRegistrationProductionValidationError::ProducerMismatch);
        }

        let path = WirePath::root();
        let actual = encode_canonical_temporary(&self, &path)?;
        let identities = self
            .identities
            .validate(foundation, digests)
            .map_err(StrongRegistrationProductionValidationError::Identities)?;
        let initialization_definitions =
            initialization_definitions.with_local_foundation(foundation, &identities, digests)?;
        let safepoints = validate_safepoints(self.safepoints, foundation, &identities)?;
        let callable_runtime_scans =
            validate_callable_runtime_scans(self.callable_runtime_scans, foundation)?;
        let types = validate_type_registration_constituents_v2(
            self.types,
            target,
            foundation,
            &identities,
            type_definitions,
            digests,
        )?;
        let immortal_objects = validate_immortal_objects(
            self.immortal_objects,
            target,
            foundation,
            &identities,
            Some(type_definitions),
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
        if actual != encode_canonical_temporary(&surface, &path)? {
            return Err(StrongRegistrationProductionValidationError::SurfaceMismatch);
        }
        Ok(surface)
    }
}
