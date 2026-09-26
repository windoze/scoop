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
}

pub(super) struct ClosedDependency {
    pub relation: MirTypeBridgeDependencyV1,
    pub dependency: usize,
}

pub(super) fn close<'a, E>(
    local: LocalView<'_>,
    dependencies: &[MirTypeBridgeDependencyViewV1<'a>],
    committed: &[MirTypeBridgeDependencyV1],
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
) -> Result<Vec<SelectedMirTypeEntryV1<'a>>, MirTypeBridgeSectionError<E>> {
    let closed = close_views(local, dependencies, committed, graph, types)?;
    let mut selected = reserve(closed.len())?;
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
) -> Result<Vec<ClosedDependency>, MirTypeBridgeSectionError<E>> {
    let count = dependencies
        .len()
        .checked_add(1)
        .ok_or(MirTypeBridgeSectionError::ArithmeticOverflow)?;
    let mut views = reserve(count)?;
    views.push(local);
    views.extend_from_slice(dependencies);
    let index = TargetIndex::build(&views)?;
    let mut pending = Vec::new();
    local.targets(|target| push(&mut pending, Pending { owner: 0, target }))?;

    if committed.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MirTypeBridgeSectionError::NonCanonicalCommittedUses);
    }
    for relation in committed {
        if relation.provider() == local.provider {
            return Err(MirTypeBridgeSectionError::SelectedCurrentProvider);
        }
        let owner = index.owner(relation.target())?;
        if views[owner].provider != relation.provider() {
            return Err(MirTypeBridgeSectionError::MissingTarget(*relation));
        }
        push(
            &mut pending,
            Pending {
                owner,
                target: relation.target(),
            },
        )?;
    }
    for usage in local.exports.initialization_uses().records() {
        let edges = MirTypeBridgeSemanticReferencesV1::of_initialization_use(usage, graph)?;
        for target in edges.targets() {
            let owner = index.owner(*target)?;
            push(
                &mut pending,
                Pending {
                    owner,
                    target: *target,
                },
            )?;
        }
    }
    let mut visited = HashSet::new();
    let mut selected = Vec::new();
    while let Some(node) = pending.pop() {
        let path = WirePath::root();

        if visited.contains(&node.target) {
            continue;
        }

        scoop_wire::allocation::try_reserve_set(&mut visited, 1, &path)?;
        visited.insert(node.target);
        let relation = MirTypeBridgeDependencyV1::new(views[node.owner].provider, node.target);
        let record = views[node.owner]
            .record(node.target)
            .ok_or(MirTypeBridgeSectionError::MissingTarget(relation))?;
        if node.owner != 0 {
            scoop_wire::allocation::try_reserve(&mut selected, 1, &path)?;
            selected.push(ClosedDependency {
                relation,
                dependency: node.owner - 1,
            });
        }

        let edges = record.references(graph, types)?;
        for target in edges.targets() {
            let owner = index.owner(*target)?;
            push(
                &mut pending,
                Pending {
                    owner,
                    target: *target,
                },
            )?;
        }
        if let MirTypeBridgeSemanticRecordV1::InitializationUnit(unit) = record {
            let uses = views[node.owner].exports.initialization_uses().records();

            for usage in uses
                .iter()
                .filter(|usage| usage.local_unit() == unit.unit())
            {
                let edges = MirTypeBridgeSemanticReferencesV1::of_initialization_use(usage, graph)?;
                for target in edges.targets() {
                    let owner = index.owner(*target)?;
                    push(
                        &mut pending,
                        Pending {
                            owner,
                            target: *target,
                        },
                    )?;
                }
            }
        }
    }

    selected.sort_unstable_by_key(|entry| entry.relation);
    Ok(selected)
}

fn push<E>(pending: &mut Vec<Pending>, value: Pending) -> Result<(), MirTypeBridgeSectionError<E>> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(pending, 1, &path)?;
    pending.push(value);
    Ok(())
}
