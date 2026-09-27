//! Rebuild the complete graph from checked runtime registration semantics.

use super::*;

pub fn replay_digest_finalization_plan_v2(
    foundation: &ConeLirFoundation,
    registrations: &crate::StrongRegistrationProductionSurfaceV2,
    entry: &EntryProductionSourceV1,
) -> Result<DigestFinalizationPlanV1, DigestProjectionError> {
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
            return Err(DigestProjectionError::ProducerMismatch {
                module: actual,
                foundation: producer,
            });
        }
    }

    let result = DigestGraphWriter::new(foundation).project_registrations(registrations, entry)?;

    Ok(result)
}
