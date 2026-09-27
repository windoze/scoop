//! Rebuild the complete graph from checked runtime registration semantics.

use super::*;

pub fn replay_strong_digest_finalization_plan_v2(
    foundation: &ConeLirFoundation,
    registrations: &crate::StrongRegistrationProductionSurfaceV2,
    entry: &EntryProductionSourceV1,
) -> Result<StrongDigestFinalizationPlanV1, StrongDigestProjectionError> {
    let producer = foundation.producer();
    for actual in [
        registrations.types().producer(),
        registrations.callables().producer(),
        registrations.safepoints().producer(),
        registrations.immortal_objects().producer(),
        registrations.static_storages().producer(),
        registrations.initialization_units().producer(),
    ] {
        if actual != producer {
            return Err(StrongDigestProjectionError::ProducerMismatch {
                module: actual,
                foundation: producer,
            });
        }
    }

    let result = DigestGraphWriter::new(foundation).project_registrations(registrations, entry)?;

    Ok(result)
}
