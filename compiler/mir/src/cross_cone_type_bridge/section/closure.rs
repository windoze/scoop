use super::selection::SelectedMirTypeEntryV1;
use super::view::LocalView;
use super::*;
use std::collections::HashSet;

mod index;
use index::TargetIndex;

#[derive(Clone, Copy)]
struct Pending {
    owner: usize,
    target: MirTypeBridgeTargetV1,
    depth: u64,
}

pub(super) struct ClosedDependency {
    pub relation: MirTypeBridgeDependencyV1,
    pub dependency: usize,
}

pub(super) fn close<'a, E>(
    local: LocalView<'_>,
    dependencies: &[&'a CrossConeMirTypeBridgeSectionV1<'a>],
    committed: &[MirTypeBridgeDependencyV1],
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<SelectedMirTypeEntryV1<'a>>, MirTypeBridgeSectionError<E>> {
    let mut views = reserve(dependencies.len(), meter)?;
    views.extend(dependencies.iter().map(|section| section.view()));
    let closed = close_views(local, &views, committed, graph, types, meter)?;
    let mut selected = reserve(closed.len(), meter)?;
    for entry in closed {
        selected.push(SelectedMirTypeEntryV1 {
            relation: entry.relation,
            terminal: dependencies[entry.dependency],
        });
    }
    Ok(selected)
}

pub(super) fn close_views<E>(
    local: LocalView<'_>,
    dependencies: &[LocalView<'_>],
    committed: &[MirTypeBridgeDependencyV1],
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<ClosedDependency>, MirTypeBridgeSectionError<E>> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut views = reserve(count, meter)?;
    views.push(local);
    views.extend_from_slice(dependencies);
    let index = TargetIndex::build(&views, meter)?;
    let mut pending = Vec::new();
    local.targets(|target| {
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
    meter.check_table_entries(committed.len() as u64, &WirePath::root())?;
    meter.charge_work(committed.len() as u64, &WirePath::root())?;
    if committed.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MirTypeBridgeSectionError::NonCanonicalCommittedUses);
    }
    for relation in committed {
        if relation.provider() == local.provider {
            return Err(MirTypeBridgeSectionError::SelectedCurrentProvider);
        }
        let owner = index.owner(relation.target(), meter)?;
        if views[owner].provider != relation.provider() {
            return Err(MirTypeBridgeSectionError::MissingTarget(*relation));
        }
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
    for usage in local.exports.initialization_uses().records() {
        let edges = MirTypeBridgeSemanticReferencesV1::of_initialization_use(usage, graph, meter)?;
        for target in edges.targets() {
            let owner = index.owner(*target, meter)?;
            push(
                &mut pending,
                Pending {
                    owner,
                    target: *target,
                    depth: 1,
                },
                meter,
            )?;
        }
    }
    let mut visited = HashSet::new();
    let mut selected = Vec::new();
    while let Some(node) = pending.pop() {
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
        if visited.contains(&node.target) {
            continue;
        }
        meter.check_semantic_depth(node.depth, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.try_reserve_set_slots(&mut visited, 1, &path)?;
        visited.insert(node.target);
        let relation = MirTypeBridgeDependencyV1::new(views[node.owner].provider, node.target);
        let record = views[node.owner]
            .record(node.target)
            .ok_or(MirTypeBridgeSectionError::MissingTarget(relation))?;
        if node.owner != 0 {
            meter.try_reserve_collection_slots(&mut selected, 1, &path)?;
            selected.push(ClosedDependency {
                relation,
                dependency: node.owner - 1,
            });
        }
        let next_depth = node
            .depth
            .checked_add(1)
            .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
        let edges = record.references(graph, types, meter)?;
        for target in edges.targets() {
            let owner = index.owner(*target, meter)?;
            push(
                &mut pending,
                Pending {
                    owner,
                    target: *target,
                    depth: next_depth,
                },
                meter,
            )?;
        }
        if let MirTypeBridgeSemanticRecordV1::InitializationUnit(unit) = record {
            let uses = views[node.owner].exports.initialization_uses().records();
            meter.charge_work(uses.len() as u64, &path)?;
            for usage in uses
                .iter()
                .filter(|usage| usage.local_unit() == unit.unit())
            {
                let edges =
                    MirTypeBridgeSemanticReferencesV1::of_initialization_use(usage, graph, meter)?;
                for target in edges.targets() {
                    let owner = index.owner(*target, meter)?;
                    push(
                        &mut pending,
                        Pending {
                            owner,
                            target: *target,
                            depth: next_depth,
                        },
                        meter,
                    )?;
                }
            }
        }
    }
    sort_work(selected.len(), meter)?;
    selected.sort_unstable_by_key(|entry| entry.relation);
    Ok(selected)
}

fn push<E>(
    pending: &mut Vec<Pending>,
    value: Pending,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSectionError<E>> {
    let path = WirePath::root();
    meter.charge_edges(1, &path)?;
    meter.check_table_entries(pending.len() as u64 + 1, &path)?;
    meter.try_reserve_collection_slots(pending, 1, &path)?;
    pending.push(value);
    Ok(())
}
