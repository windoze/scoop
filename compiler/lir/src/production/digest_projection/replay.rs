//! Rebuild the complete graph from checked runtime registration semantics.

use scoop_wire::{WirePath, encoded_length};

use super::*;

pub fn replay_strong_digest_finalization_plan_v2(
    foundation: &OdrFreeLirFoundation,
    registrations: &crate::StrongRegistrationProductionSurfaceV2,
    entry: &EntryProductionSourceV1,
    meter: &mut BudgetMeter,
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
    let counts = [
        foundation.callable_bodies().len(),
        registrations.types().registrations().len(),
        registrations.safepoints().registrations().len(),
        registrations.immortal_objects().registrations().len(),
        registrations.static_storages().registrations().len(),
        registrations.initialization_units().registrations().len(),
    ];
    charge_projection(foundation, counts, meter)?;
    let result = DigestGraphWriter::new(foundation).project_registrations(registrations, entry)?;
    let length = encoded_length(&result).map_err(|_| StrongDigestProjectionError::Encoding)?;
    meter.charge_work(length, &WirePath::root())?;
    Ok(result)
}

pub(super) fn charge_projection(
    foundation: &OdrFreeLirFoundation,
    counts: [usize; 6],
    meter: &mut BudgetMeter,
) -> Result<(), StrongDigestProjectionError> {
    let path = WirePath::root();
    let mut records = 1_u64;
    for count in counts {
        meter.check_table_entries(count as u64, &path)?;
        records = records.saturating_add(count as u64);
    }
    // Each role adds at most eight nodes, eight inputs and six patch records.
    // The bound also covers map/set indexes, canonical records and hashing.
    let nodes = records.saturating_mul(8);
    let edges = records.saturating_mul(8);
    meter.charge_nodes(nodes, &path)?;
    meter.charge_edges(edges, &path)?;
    meter.charge_collection_slots(records.saturating_mul(64), &path)?;
    meter.charge_owned_bytes(records.saturating_mul(8192), &path)?;
    meter.charge_sha256(records.saturating_mul(8192), &path)?;
    let search = (foundation.definition_plans().len() as u64)
        .saturating_add(foundation.definition_atoms().len() as u64)
        .saturating_add(foundation.callable_bodies().len() as u64)
        .saturating_add(nodes)
        .saturating_add(edges)
        .saturating_add(1);
    meter.charge_work(records.saturating_mul(search).saturating_mul(16), &path)?;
    meter.charge_stable_kahn(nodes, edges, &path)?;
    Ok(())
}
