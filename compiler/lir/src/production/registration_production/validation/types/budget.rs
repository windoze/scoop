//! Reader-side costs are charged before replay allocates or clones records.

use super::*;

pub(super) fn charge_plan_replay(
    count: usize,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    digests: &StrongDigestFinalizationPlanV1,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    let count = count as u64;
    meter.check_table_entries(count, &path)?;
    meter.charge_nodes(count, &path)?;
    let mut work = (foundation.definition_plans().len() as u64)
        .saturating_add(foundation.definition_atoms().len() as u64)
        .saturating_add(foundation.symbol_requests().len() as u64)
        .saturating_add(foundation.layouts().len() as u64)
        .saturating_add(foundation.scans().len() as u64)
        .saturating_add(identities.type_registrations().len() as u64)
        .saturating_add(digests.nodes().len() as u64);
    meter.charge_work(digests.nodes().len() as u64, &path)?;
    for node in digests.nodes() {
        work = work
            .saturating_add(node.direct_inputs().len() as u64)
            .saturating_add(node.patch_intents().len() as u64);
    }
    // Definition replay scans these tables for every registration and may
    // collect each matching atom/digest input into a comparison set.
    meter.charge_work(
        count
            .saturating_mul(work.saturating_add(1))
            .saturating_mul(32),
        &path,
    )?;
    meter.charge_collection_slots(
        count.saturating_mul(
            (foundation.definition_atoms().len() as u64)
                .saturating_add(digests.nodes().len() as u64)
                .saturating_add(8),
        ),
        &path,
    )
}

pub(super) fn charge_record<R: TypeReferences>(
    record: &DecodedTypeFor<R>,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    meter.charge_owned_bytes(record.diagnostic_name.len() as u64, &path)?;
    meter.charge_collection_slots(
        (record.itables.len() as u64)
            .saturating_mul(4)
            .saturating_add(1),
        &path,
    )?;
    meter.charge_work(record.itables.len() as u64, &path)?;
    meter.charge_collection_slots(record.vtable.slots.len() as u64, &path)?;
    for table in &record.itables {
        meter.charge_collection_slots(table.slots.len() as u64, &path)?;
    }
    charge_scan(&record.instance_shape.object_scan, 1, meter)?;
    charge_scan(&record.instance_shape.inline_scan, 1, meter)
}

fn charge_scan(
    scan: &DecodedRefScan,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    // Shape replay and expected-plan construction retain at most six copies.
    meter.charge_nodes(6, &path)?;
    meter.charge_work(depth.saturating_add(1).saturating_mul(16), &path)?;
    match scan {
        DecodedRefScan::None => Ok(()),
        DecodedRefScan::References(offsets) => {
            meter.charge_collection_slots((offsets.len() as u64).saturating_mul(6), &path)
        }
        DecodedRefScan::Sequence(parts) => {
            meter.charge_collection_slots((parts.len() as u64).saturating_mul(6), &path)?;
            for part in parts {
                charge_scan(part, depth.saturating_add(1), meter)?;
            }
            Ok(())
        }
        DecodedRefScan::Array { element, .. } => {
            charge_scan(element, depth.saturating_add(1), meter)
        }
    }
}
