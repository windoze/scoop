use std::collections::BTreeSet;

use scoop_identity::PersistentExactTypeId;
use scoop_wire::{WireError, WirePath};

/// Traverses source nominal edges without substituting object backing classes.
/// Every lookup, including an equal receiver and owner, must resolve a real node.
pub(crate) fn is_nominal_ancestor<E, P>(
    receiver: PersistentExactTypeId,
    owner: PersistentExactTypeId,

    path: &WirePath,
    mut parents: impl FnMut(PersistentExactTypeId) -> Result<P, E>,
) -> Result<bool, E>
where
    E: From<WireError>,
    P: IntoIterator<Item = PersistentExactTypeId>,
{
    let mut pending = Vec::new();
    scoop_wire::allocation::try_reserve(&mut pending, 1, path)?;
    pending.push((receiver, 1));
    let mut seen = BTreeSet::new();
    while let Some((current, depth)) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }

        seen.insert(current);
        let parents = parents(current)?;
        if current == owner {
            return Ok(true);
        }
        for parent in parents {
            scoop_wire::allocation::try_reserve(&mut pending, 1, path)?;
            pending.push((parent, depth + 1));
        }
    }
    Ok(false)
}
