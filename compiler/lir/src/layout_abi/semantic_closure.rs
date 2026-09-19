use super::*;
use std::collections::HashSet;

mod index;
mod references;

use index::LayoutAbiTargetIndex;

#[derive(Clone, Copy)]
struct Pending {
    owner: usize,
    target: LayoutAbiSemanticTargetV1,
    depth: u64,
}

pub(super) fn close<'a>(
    consumer: ConeIdentity,
    local: &'a LayoutAbiExportConstituentsV1,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    roots: &[LayoutAbiDependencyV1],
    meter: &mut BudgetMeter,
) -> Result<Vec<LayoutAbiDependencyV1>, LayoutAbiSemanticClosureError> {
    let path = WirePath::root();
    meter.check_table_entries(roots.len() as u64, &path)?;
    meter.charge_work(roots.len() as u64, &path)?;
    if roots.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LayoutAbiSemanticClosureError::NonCanonicalRoots);
    }
    let mut views = Vec::new();
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(LayoutAbiSemanticClosureError::ArithmeticOverflow)?;
    meter.try_reserve_collection_slots(&mut views, count, &path)?;
    views.push(local);
    views.extend_from_slice(dependencies);
    let index = LayoutAbiTargetIndex::build(&views, meter)?;
    let mut pending = Vec::new();
    local.visit_targets(|target| {
        push(
            &mut pending,
            Pending {
                owner: 0,
                target,
                depth: 1,
            },
            meter,
        )
    })?;
    for relation in roots {
        if relation.provider() == consumer {
            return Err(LayoutAbiSemanticClosureError::CurrentProvider(
                relation.target(),
            ));
        }
        let owner = index.owner(relation.target(), meter)?;
        require_provider(&views, owner, relation.provider(), relation.target())?;
        push(
            &mut pending,
            Pending {
                owner,
                target: relation.target(),
                depth: 1,
            },
            meter,
        )?;
    }
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    while let Some(next) = pending.pop() {
        meter.check_semantic_depth(next.depth, &path)?;
        meter.charge_nodes(1, &path)?;
        if !seen.contains(&(next.owner, next.target)) {
            meter.try_reserve_set_slots(&mut seen, 1, &path)?;
            seen.insert((next.owner, next.target));
        } else {
            continue;
        }
        let record = views[next.owner]
            .record(next.target)
            .ok_or(LayoutAbiSemanticClosureError::MissingTarget(next.target))?;
        if next.owner != 0 {
            meter.try_reserve_collection_slots(&mut selected, 1, &path)?;
            selected.push(LayoutAbiDependencyV1::new(
                views[next.owner].provider(),
                next.target,
            ));
        }
        references::enqueue(
            record,
            next.owner,
            next.depth,
            &views,
            &index,
            &mut pending,
            meter,
        )?;
    }
    sort_work(selected.len(), meter)?;
    selected.sort_unstable();
    Ok(selected)
}

fn push(
    pending: &mut Vec<Pending>,
    value: Pending,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    meter.charge_edges(1, &WirePath::root())?;
    meter.try_reserve_collection_slots(pending, 1, &WirePath::root())?;
    pending.push(value);
    Ok(())
}

fn require_provider(
    views: &[&LayoutAbiExportConstituentsV1],
    owner: usize,
    expected: ConeIdentity,
    target: LayoutAbiSemanticTargetV1,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let actual = views[owner].provider();
    if actual == expected {
        Ok(())
    } else {
        Err(LayoutAbiSemanticClosureError::Provider {
            target,
            expected,
            actual,
        })
    }
}

fn sort_work(count: usize, meter: &mut BudgetMeter) -> Result<(), LayoutAbiSemanticClosureError> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    let levels = usize::BITS - count.max(1).saturating_sub(1).leading_zeros();
    for _ in 0..levels {
        meter.charge_work(count as u64, &WirePath::root())?;
    }
    Ok(())
}
