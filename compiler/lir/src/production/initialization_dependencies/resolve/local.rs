//! Local unit definitions come from this section's checked physical identities.

use super::*;
use crate::{
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongRegistrationProductionValidationError,
};

impl StrongInitializationDefinitionCatalogV2 {
    /// The input catalog contains dependency definitions only. Local entries
    /// are reconstructed here before the complete registration/DAG replay.
    pub(in crate::production) fn with_local_foundation(
        &self,
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        digests: &StrongDigestFinalizationPlanV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, StrongRegistrationProductionValidationError> {
        if self.producer != foundation.producer() {
            return Err(StrongRegistrationProductionValidationError::ProducerMismatch);
        }
        let path = WirePath::root();
        let mut definitions = Vec::new();
        meter.charge_work(self.definitions.len() as u64, &path)?;
        meter.try_reserve_collection_slots(&mut definitions, self.definitions.len(), &path)?;
        for definition in &self.definitions {
            if definition.provider() == self.producer {
                return Err(
                    InitializationDependencyResolutionError::CurrentConeDefinition(
                        definition.unit(),
                    )
                    .into(),
                );
            }
            definitions.push(*definition);
        }
        meter.try_reserve_collection_slots(
            &mut definitions,
            identities.initialization_units().len(),
            &path,
        )?;
        for identity in identities.initialization_units() {
            definitions.push(StrongInitializationUnitDefinitionRefV2::from_foundation(
                identity.semantic_id(),
                foundation,
                identities,
                digests,
                meter,
            )?);
        }
        Ok(Self::new(self.producer, &definitions, meter)?)
    }
}
