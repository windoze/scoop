use std::collections::BTreeSet;

use scoop_identity::PersistentExactTypeId;
use scoop_wire::{BudgetMeter, WireError, WirePath};

/// Traverses source nominal edges without substituting object backing classes.
/// Every lookup, including an equal receiver and owner, must resolve a real node.
pub(crate) fn is_nominal_ancestor<E, P>(
    receiver: PersistentExactTypeId,
    owner: PersistentExactTypeId,
    meter: &mut BudgetMeter,
    path: &WirePath,
    mut parents: impl FnMut(PersistentExactTypeId, &mut BudgetMeter) -> Result<P, E>,
) -> Result<bool, E>
where
    E: From<WireError>,
    P: IntoIterator<Item = PersistentExactTypeId>,
{
    let mut pending = Vec::new();
    meter.try_reserve_collection_slots(&mut pending, 1, path)?;
    pending.push((receiver, 1));
    let mut seen = BTreeSet::new();
    while let Some((current, depth)) = pending.pop() {
        meter.charge_work(1 + u64::from(seen.len().max(1).ilog2()), path)?;
        if seen.contains(&current) {
            continue;
        }
        meter.check_semantic_depth(depth, path)?;
        meter.charge_nodes(1, path)?;
        meter.check_table_entries(seen.len() as u64 + 1, path)?;
        meter.charge_collection_slots(1, path)?;
        seen.insert(current);
        let parents = parents(current, meter)?;
        if current == owner {
            return Ok(true);
        }
        for parent in parents {
            meter.charge_edges(1, path)?;
            meter.try_reserve_collection_slots(&mut pending, 1, path)?;
            pending.push((parent, depth + 1));
        }
    }
    Ok(false)
}
