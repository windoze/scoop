//! Preflight the existing physical plan constructors using one shared meter.

use super::*;

pub(super) fn charge_replay(
    foundation: &OdrFreeLirFoundation,
    digests: &StrongDigestFinalizationPlanV1,
    sources: &[SourceDeclarationKey],
    initialization_abi: Option<&CallableAbiRecordV1>,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let definitions = foundation.definition_plans().len() as u64;
    let atoms = foundation.definition_atoms().len() as u64;
    let nodes = digests.nodes().len() as u64;
    let source_count = sources.len() as u64;
    let entries = definitions
        .saturating_add(atoms)
        .saturating_add(nodes)
        .saturating_add(foundation.symbol_requests().len() as u64)
        .saturating_add(foundation.layouts().len() as u64)
        .saturating_add(foundation.scans().len() as u64)
        .saturating_add(foundation.bridge_units().len() as u64)
        .saturating_add(foundation.bridge_atoms().len() as u64)
        .saturating_add(source_count);
    meter.charge_nodes(entries, &path)?;
    meter.charge_work(nodes, &path)?;
    let mut edges = 0_u64;
    for node in digests.nodes() {
        edges = edges
            .saturating_add(node.direct_inputs().len() as u64)
            .saturating_add(node.patch_intents().len() as u64);
    }
    meter.charge_edges(edges, &path)?;
    meter.charge_collection_slots(
        entries
            .saturating_mul(16)
            .saturating_add(edges.saturating_mul(4)),
        &path,
    )?;
    // Covers per-plan atom scans, digest graph indexes, root role lookup and
    // definition/bridge canonical sorting. The constructors retain no input
    // bytes and never inspect dependency bodies.
    meter.charge_work(
        entries
            .saturating_mul(entries.saturating_add(edges).saturating_add(1))
            .saturating_mul(16),
        &path,
    )?;
    meter.charge_stable_kahn(nodes, edges, &path)?;
    for source in sources {
        // Source keys may contain owned declaration names. Charge canonical
        // hashing and copies before the shape support closure uses them.
        let bytes = encode_canonical_temporary_with_meter(source, meter, &path)?;
        meter.charge_sha256(bytes.len() as u64, &path)?;
    }
    if initialization_abi.is_some() {
        let count = 1_u64;
        meter.charge_collection_slots(count.saturating_mul(4), &path)?;
        meter.charge_work(
            count
                .saturating_mul(entries.saturating_add(1))
                .saturating_mul(8),
            &path,
        )?;
    }
    Ok(())
}
