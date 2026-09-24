//! Preflight the existing physical plan constructors using one shared meter.

use super::*;

pub(super) fn charge_replay(
    foundation: &OdrFreeLirFoundation,
    direct_dependencies: &[ConeIdentity],
    digests: &StrongDigestFinalizationPlanV1,
    sources: &[SourceDeclarationKey],
    initialization_abi: Option<&CallableAbiRecordV1>,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let definitions = foundation.definition_plans().len() as u64;
    let atoms = foundation.definition_atoms().len() as u64;
    let symbols = foundation.symbol_requests().len() as u64;
    let layouts = foundation.layouts().len() as u64;
    let scans = foundation.scans().len() as u64;
    let nodes = digests.nodes().len() as u64;
    let source_count = sources.len() as u64;
    let entries = definitions
        .saturating_add(direct_dependencies.len() as u64)
        .saturating_add(atoms)
        .saturating_add(nodes)
        .saturating_add(symbols)
        .saturating_add(layouts)
        .saturating_add(scans)
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
    // Object, symbol and generated-bridge plans group atoms in indexes. Their
    // three construction passes and sorting are linear-logarithmic; only the
    // primary-symbol check scans the symbol table for every definition.
    meter.charge_work(
        entries
            .saturating_mul(12 * 64)
            .saturating_add(definitions.saturating_mul(symbols)),
        &path,
    )?;
    crate::production::digests::budget::charge_validation(digests.nodes(), foundation, meter)?;
    // Each source has at most four exact shapes. Every shape queries one
    // value layout/scan and four definition, atom and symbol relations.
    let shapes = source_count.saturating_mul(4);
    let shape_search = (foundation.materialized_exact_types().len() as u64)
        .saturating_add(layouts)
        .saturating_add(scans)
        .saturating_add(definitions)
        .saturating_add(
            definitions
                .saturating_add(atoms)
                .saturating_add(symbols)
                .saturating_mul(4),
        );
    meter.charge_work(shapes.saturating_mul(shape_search), &path)?;
    meter.charge_collection_slots(
        shapes.saturating_mul(layouts.saturating_add(scans).saturating_add(atoms)),
        &path,
    )?;
    // Image and executable-entry construction perform a fixed set of role
    // queries, independent of the number of unrelated production records.
    meter.charge_work(entries.saturating_add(edges).saturating_mul(24), &path)?;
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
