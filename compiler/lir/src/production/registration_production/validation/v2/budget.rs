//! Charge each registration role before it allocates or traverses tables.

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
    let definitions = foundation.definition_plans().len() as u64;
    let atoms = foundation.definition_atoms().len() as u64;
    let symbols = foundation.symbol_requests().len() as u64;
    let nodes = digests.nodes().len() as u64;
    let bodies = foundation.callable_bodies().len() as u64;
    let mut records = 0_u64;
    for count in counts {
        meter.check_table_entries(count as u64, &path)?;
        records = records.saturating_add(count as u64);
    }
    meter.charge_nodes(records.saturating_add(definitions), &path)?;
    meter.charge_work(nodes, &path)?;
    let mut digest_entries = nodes;
    for node in digests.nodes() {
        digest_entries = digest_entries
            .saturating_add(node.direct_inputs().len() as u64)
            .saturating_add(node.patch_intents().len() as u64);
    }
    let physical = definitions
        .saturating_add(atoms)
        .saturating_add(symbols)
        .saturating_add(digest_entries)
        .saturating_add(1);
    let search = physical
        .saturating_add(foundation.layouts().len() as u64)
        .saturating_add(foundation.scans().len() as u64)
        .saturating_add(bodies)
        .saturating_add(foundation.safepoint_sites().len() as u64)
        .saturating_add(foundation.safepoint_mappings().len() as u64)
        .saturating_add(external.bridges().len() as u64)
        .saturating_add(records);
    // Identity replay scans all definitions but looks up only one digest
    // per registration role. Sorting compares fixed-size typed identities.
    meter.charge_work(
        definitions
            .saturating_mul(nodes.saturating_add(64))
            .saturating_add(records.saturating_mul(64)),
        &path,
    )?;
    meter.charge_collection_slots(
        records
            .saturating_mul(search.saturating_add(16))
            .saturating_add(definitions.saturating_mul(4)),
        &path,
    )?;
    // Type and initialization constituents charge their physical plan
    // reconstruction, including the final surface pass, in their own meters.
    // Other roles have a fixed number of physical queries per registration.
    let callables = bodies.max(decoded.callables.len() as u64);
    meter.charge_work(callables.saturating_mul(physical.saturating_mul(8)), &path)?;
    let safepoint_search = physical
        .saturating_mul(8)
        .saturating_add((foundation.safepoint_sites().len() as u64).saturating_mul(2))
        .saturating_add((foundation.safepoint_mappings().len() as u64).saturating_mul(2));
    meter.charge_work(
        (decoded.safepoints.len() as u64).saturating_mul(safepoint_search),
        &path,
    )?;
    let immortal_search = physical
        .saturating_mul(16)
        .saturating_add(decoded.types.len() as u64)
        .saturating_add(external.bridges().len() as u64);
    meter.charge_work(
        (decoded.immortal_objects.len() as u64).saturating_mul(immortal_search),
        &path,
    )?;
    let storage_search = physical
        .saturating_mul(16)
        .saturating_add((foundation.layouts().len() as u64).saturating_mul(2))
        .saturating_add((foundation.scans().len() as u64).saturating_mul(2));
    meter.charge_work(
        (decoded.static_storages.len() as u64).saturating_mul(storage_search),
        &path,
    )?;
    meter.charge_work(
        (decoded.callable_runtime_scans.len() as u64).saturating_mul(bodies.saturating_add(64)),
        &path,
    )?;
    for callable in &decoded.callable_runtime_scans {
        let count = callable.atoms.len() as u64;
        meter.charge_collection_slots(count.saturating_mul(4), &path)?;
        meter.charge_work(count.saturating_mul(atoms.saturating_add(64)), &path)?;
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
            let relocation_search = (decoded.immortal_objects.len() as u64)
                .saturating_add(symbols)
                .saturating_add(atoms)
                .saturating_add(16);
            meter.charge_work(
                relocations
                    .saturating_mul(relocation_search)
                    .saturating_mul(2),
                &path,
            )?;
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
