use super::*;
use std::collections::BTreeSet;

/// Receiver ancestry includes interface edges. Object backing classes never
/// substitute for source object IDs in this source-level lookup relation.
pub(super) fn validate<E>(
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    receiver: PersistentExactTypeId,
    owner: PersistentExactTypeId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error<E>> {
    let mut pending = Vec::new();
    meter.try_reserve_collection_slots(&mut pending, 1, path)?;
    pending.push((receiver, 1));
    let mut seen = BTreeSet::new();
    while let Some((current, depth)) = pending.pop() {
        meter.charge_work((seen.len() as u64 + 1).ilog2() as u64 + 1, path)?;
        if seen.contains(&current) {
            continue;
        }
        meter.check_semantic_depth(depth, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_collection_slots(1, path)?;
        seen.insert(current);
        let node = graph.get(current).ok_or(Error::Receiver)?;
        if current == owner {
            return Ok(());
        }
        let edges = node.edges();
        meter.charge_edges(edges.direct_interfaces().len() as u64 + 1, path)?;
        meter.try_reserve_collection_slots(
            &mut pending,
            edges.direct_interfaces().len() + 1,
            path,
        )?;
        pending.extend(
            edges
                .direct_interfaces()
                .iter()
                .map(|exact| (*exact, depth + 1)),
        );
        if let DirectClassBaseV1::ClassBase { exact } = edges.direct_base() {
            pending.push((exact, depth + 1));
        }
    }
    Err(Error::Receiver)
}
