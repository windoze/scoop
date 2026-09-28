use super::*;
use std::collections::HashSet;

mod index;
mod references;

use index::LayoutAbiTargetIndex;

#[derive(Clone, Copy)]
struct Pending {
    owner: usize,
    target: LayoutAbiSemanticTargetV1,
}

pub(super) fn close<'a>(
    consumer: ConeIdentity,
    local: &'a LayoutAbiExportConstituentsV1,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    roots: &[LayoutAbiDependencyV1],
) -> Result<Vec<LayoutAbiDependencyV1>, LayoutAbiSemanticClosureError> {
    validate_roots(roots)?;
    let path = WirePath::root();
    let mut views = Vec::new();
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(LayoutAbiSemanticClosureError::ArithmeticOverflow)?;
    scoop_wire::allocation::try_reserve(&mut views, count, &path)?;
    views.push(local);
    views.extend_from_slice(dependencies);
    let index = LayoutAbiTargetIndex::build(&views)?;
    let mut pending = Vec::new();
    local.visit_targets(|target| push(&mut pending, Pending { owner: 0, target }))?;
    enqueue_roots(consumer, &index, roots, &mut pending)?;
    collect(&views, &index, pending, Some(0))
}

/// Closes dependency roots before local exports exist. The roots come from
/// the same independent MIR-to-LIR authority later used to validate the
/// completed section, so Strong V2 production cannot select arbitrary records
/// from an otherwise valid terminal provider.
pub(super) fn close_external(
    consumer: ConeIdentity,
    dependencies: &[&LayoutAbiExportConstituentsV1],
    roots: &[LayoutAbiDependencyV1],
) -> Result<Vec<LayoutAbiDependencyV1>, LayoutAbiSemanticClosureError> {
    validate_roots(roots)?;
    let mut views = Vec::new();
    scoop_wire::allocation::try_reserve(&mut views, dependencies.len(), &WirePath::root())?;
    views.extend_from_slice(dependencies);
    let index = LayoutAbiTargetIndex::build(&views)?;
    let mut pending = Vec::new();
    enqueue_roots(consumer, &index, roots, &mut pending)?;
    collect(&views, &index, pending, None)
}

fn validate_roots(roots: &[LayoutAbiDependencyV1]) -> Result<(), LayoutAbiSemanticClosureError> {
    if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LayoutAbiSemanticClosureError::NonCanonicalRoots);
    }
    Ok(())
}

fn enqueue_roots(
    consumer: ConeIdentity,
    index: &LayoutAbiTargetIndex,
    roots: &[LayoutAbiDependencyV1],
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    for relation in roots {
        if relation.provider() == consumer {
            return Err(LayoutAbiSemanticClosureError::CurrentProvider(
                relation.target(),
            ));
        }
        let owner = index.owner(relation.target(), Some(relation.provider()))?;
        push(
            pending,
            Pending {
                owner,
                target: relation.target(),
            },
        )?;
    }
    Ok(())
}

fn collect(
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    mut pending: Vec<Pending>,
    local_owner: Option<usize>,
) -> Result<Vec<LayoutAbiDependencyV1>, LayoutAbiSemanticClosureError> {
    let path = WirePath::root();
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    while let Some(next) = pending.pop() {
        if !seen.contains(&(next.owner, next.target)) {
            scoop_wire::allocation::try_reserve_set(&mut seen, 1, &path)?;
            seen.insert((next.owner, next.target));
        } else {
            continue;
        }
        let record = views[next.owner]
            .record(next.target)
            .ok_or(LayoutAbiSemanticClosureError::MissingTarget(next.target))?;
        if Some(next.owner) != local_owner {
            scoop_wire::allocation::try_reserve(&mut selected, 1, &path)?;
            selected.push(LayoutAbiDependencyV1::new(
                views[next.owner].provider(),
                next.target,
            ));
        }
        references::enqueue(record, next.owner, views, index, &mut pending)?;
    }

    selected.sort_unstable();
    Ok(selected)
}

fn push(pending: &mut Vec<Pending>, value: Pending) -> Result<(), LayoutAbiSemanticClosureError> {
    scoop_wire::allocation::try_reserve(pending, 1, &WirePath::root())?;
    pending.push(value);
    Ok(())
}
