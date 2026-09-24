//! Charge graph lookups by owner kind instead of multiplying unrelated tables.

use scoop_identity::DigestKind;
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{DecodedStrongDigestFinalizationPlanV1, DigestNodeV1};
use crate::OdrFreeLirFoundation;

pub(in crate::production) fn charge_resolution(
    graph: &DecodedStrongDigestFinalizationPlanV1,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    charge_work(
        graph.nodes.iter().map(|node| {
            (
                node.identity.key().owner_kind(),
                node.direct_inputs.len(),
                node.patch_intents.len(),
            )
        }),
        foundation,
        true,
        meter,
    )
    .map(|_| ())
}

pub(in crate::production) fn charge_validation(
    nodes: &[DigestNodeV1],
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let (nodes, edges) = charge_work(
        nodes.iter().map(|node| {
            (
                node.kind(),
                node.direct_inputs().len(),
                node.patch_intents().len(),
            )
        }),
        foundation,
        false,
        meter,
    )?;
    meter.charge_stable_kahn(nodes, edges, &WirePath::root())
}

fn charge_work(
    records: impl ExactSizeIterator<Item = (DigestKind, usize, usize)>,
    foundation: &OdrFreeLirFoundation,
    resolve: bool,
    meter: &mut BudgetMeter,
) -> Result<(u64, u64), WireError> {
    let path = WirePath::root();
    let nodes = records.len() as u64;
    meter.charge_work(nodes, &path)?;
    let mut edges = 0_u64;
    let mut patches = 0_u64;
    let mut owners = 0_u64;
    for (kind, inputs, writers) in records {
        edges = edges.saturating_add(inputs as u64);
        patches = patches.saturating_add(writers as u64);
        let candidates = match kind {
            DigestKind::SourceSignature => foundation.callable_bodies().len(),
            DigestKind::Layout => foundation.layouts().len(),
            DigestKind::Scan => foundation.scans().len(),
            DigestKind::LirDefinition
            | DigestKind::ObjectSupport
            | DigestKind::ObjectDefinition => foundation.definition_atoms().len(),
            DigestKind::StackmapRecord => foundation.safepoint_sites().len(),
            DigestKind::StrongRegistration => foundation.definition_plans().len(),
            DigestKind::RuntimeImage | DigestKind::OdrDefinition => 1,
        };
        owners = owners.saturating_add(candidates as u64);
    }
    let patch_lookup = (foundation.definition_atoms().len() as u64).saturating_add(if resolve {
        foundation.definition_plans().len() as u64
    } else {
        0
    });
    // Resolution and validation each inspect the owner once. The known-node
    // and patch-writer indexes require only logarithmic typed-key comparisons.
    let index_entries = nodes
        .saturating_mul(4)
        .saturating_add(edges.saturating_mul(3))
        .saturating_add(patches.saturating_mul(2));
    let work = owners
        .saturating_mul(if resolve { 2 } else { 1 })
        .saturating_add(patches.saturating_mul(patch_lookup))
        .saturating_add(index_entries.saturating_mul(64));
    meter.charge_work(work, &path)?;
    Ok((nodes, edges))
}
