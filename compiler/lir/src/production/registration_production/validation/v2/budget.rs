//! Charge shared legacy reconstruction before it allocates or traverses tables.

use super::*;

pub(super) fn charge_replay(
    decoded: &DecodedStrongRegistrationProductionSurfaceV2,
    foundation: &OdrFreeLirFoundation,
    digests: &StrongDigestFinalizationPlanV1,
    external: &StrongExternalLirBridgeSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    let counts = [
        decoded.safepoints.len(),
        decoded.callables.len(),
        decoded.types.len(),
        decoded.immortal_objects.len(),
        decoded.static_storages.len(),
        decoded.initialization_units.len(),
        decoded.callable_runtime_scans.len(),
    ];
    let mut records = foundation.definition_plans().len() as u64;
    for count in counts {
        meter.check_table_entries(count as u64, &path)?;
        records = records.saturating_add(count as u64);
    }
    meter.charge_nodes(records, &path)?;
    let mut search = (foundation.definition_plans().len() as u64)
        .saturating_add(foundation.definition_atoms().len() as u64)
        .saturating_add(foundation.symbol_requests().len() as u64)
        .saturating_add(foundation.layouts().len() as u64)
        .saturating_add(foundation.scans().len() as u64)
        .saturating_add(foundation.callable_bodies().len() as u64)
        .saturating_add(foundation.safepoint_sites().len() as u64)
        .saturating_add(foundation.safepoint_mappings().len() as u64)
        .saturating_add(external.bridges().len() as u64)
        .saturating_add(digests.nodes().len() as u64)
        .saturating_add(records);
    meter.charge_work(digests.nodes().len() as u64, &path)?;
    for node in digests.nodes() {
        search = search
            .saturating_add(node.direct_inputs().len() as u64)
            .saturating_add(node.patch_intents().len() as u64);
    }
    // Includes identity-table sorting, per-registration physical lookup and
    // the sets of direct digest inputs/patch writers rebuilt by each plan.
    meter.charge_work(
        records
            .saturating_mul(search.saturating_add(1))
            .saturating_mul(64),
        &path,
    )?;
    meter.charge_collection_slots(records.saturating_mul(search.saturating_add(16)), &path)?;
    for callable in &decoded.callable_runtime_scans {
        let atoms = callable.atoms.len() as u64;
        meter.charge_collection_slots(atoms.saturating_mul(4), &path)?;
        meter.charge_work(atoms.saturating_mul(search.saturating_add(1)), &path)?;
        for atom in &callable.atoms {
            charge_scan(&atom.scan, 1, meter)?;
        }
    }
    for storage in &decoded.static_storages {
        charge_scan(&storage.semantic.scan_program, 1, meter)?;
        if let DecodedStrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template,
            immortal_relocations,
        } = &storage.semantic.initial_state
        {
            meter.charge_owned_bytes((initial_template.len() as u64).saturating_mul(3), &path)?;
            let relocations = immortal_relocations.len() as u64;
            meter.charge_collection_slots(relocations.saturating_mul(4), &path)?;
            meter.charge_work(relocations.saturating_mul(search.saturating_add(16)), &path)?;
        }
    }
    Ok(())
}

fn charge_scan(
    scan: &DecodedRefScan,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_nodes(4, &path)?;
    meter.charge_work(4, &path)?;
    match scan {
        DecodedRefScan::None => Ok(()),
        DecodedRefScan::References(offsets) => {
            meter.charge_collection_slots((offsets.len() as u64).saturating_mul(4), &path)?;
            meter.charge_work((offsets.len() as u64).saturating_mul(4), &path)
        }
        DecodedRefScan::Sequence(parts) => {
            meter.charge_collection_slots((parts.len() as u64).saturating_mul(4), &path)?;
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
