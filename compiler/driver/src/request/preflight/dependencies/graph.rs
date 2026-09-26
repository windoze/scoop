use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::LoadedConeManifest;
use scoop_slib::{ArtifactManifestSummaryV1, ConeKind, ConeSourceForm};

use super::{
    DependencyValidationResult, ExplicitDependencyArtifactInput, ExplicitDependencyRole,
    ExplicitDependencyValidationError,
};

pub(super) struct ValidatedDependencyNode<'input> {
    pub(super) input: ExplicitDependencyArtifactInput,
    pub(super) bytes: &'input [u8],
    pub(super) summary: Cow<'input, ArtifactManifestSummaryV1>,
}

pub(super) fn validate_artifact_shape(
    input: &ExplicitDependencyArtifactInput,
    summary: &ArtifactManifestSummaryV1,
    current_identity: ConeIdentity,
) -> DependencyValidationResult<()> {
    let cone = summary.cone();
    debug_assert_ne!(cone.identity(), current_identity);
    if cone.kind() != ConeKind::Library || cone.source_form() != ConeSourceForm::Manifest {
        return Err(
            ExplicitDependencyValidationError::UnsupportedArtifactShape {
                input: input.clone(),
                kind: cone.kind(),
                source_form: cone.source_form(),
            }
            .into(),
        );
    }
    Ok(())
}

pub(super) fn validate_manifest_direct_set(
    manifest: Option<&LoadedConeManifest>,
    current: ConeIdentity,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
) -> DependencyValidationResult<()> {
    let mut declared = BTreeMap::new();
    for coordinate in manifest
        .map(|manifest| {
            manifest
                .parsed()
                .semantic()
                .dependency_iter()
                .map(|(_, coordinate)| coordinate)
        })
        .into_iter()
        .flatten()
    {
        let identity = coordinate.identity().map_err(|source| {
            Box::new(ExplicitDependencyValidationError::ManifestIdentity {
                coordinate: coordinate.clone(),
                source,
            })
        })?;
        declared.insert(identity, coordinate.clone());
    }
    if current != ConeIdentity::CORE {
        declared
            .entry(ConeIdentity::CORE)
            .or_insert_with(ConeCoordinate::reserved_core);
    }
    let actual = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Direct)
        .map(|(identity, node)| (*identity, node.summary.cone().coordinate().clone()))
        .collect::<BTreeMap<_, _>>();
    if declared != actual {
        return Err(ExplicitDependencyValidationError::ManifestDirectSet {
            declared: declared.into_values().collect(),
            actual: actual.into_values().collect(),
        }
        .into());
    }
    Ok(())
}

pub(super) fn validate_dependency_records(
    current_identity: ConeIdentity,
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
) -> DependencyValidationResult<()> {
    for node in nodes.values() {
        let cone = node.summary.cone();
        for dependency in node.summary.direct_dependencies() {
            if dependency.identity() == cone.identity() {
                return Err(ExplicitDependencyValidationError::SelfDependency {
                    coordinate: cone.coordinate().clone(),
                }
                .into());
            }
            if dependency.identity() == current_identity {
                return Err(ExplicitDependencyValidationError::CycleToCurrentCone {
                    coordinate: cone.coordinate().clone(),
                    dependency: dependency.coordinate().clone(),
                }
                .into());
            }
            let Some(target) = nodes.get(&dependency.identity()) else {
                return Err(
                    ExplicitDependencyValidationError::MissingDependencyArtifact {
                        coordinate: cone.coordinate().clone(),
                        dependency: dependency.coordinate().clone(),
                    }
                    .into(),
                );
            };
            if !dependency_matches_summary(dependency, &target.summary) {
                return Err(
                    ExplicitDependencyValidationError::DependencyRecordMismatch {
                        coordinate: cone.coordinate().clone(),
                        dependency: dependency.coordinate().clone(),
                    }
                    .into(),
                );
            }
        }
    }
    Ok(())
}

fn dependency_matches_summary(
    dependency: &scoop_slib::DependencyRecord,
    summary: &ArtifactManifestSummaryV1,
) -> bool {
    let semantic = summary.semantic_fingerprints();
    dependency.coordinate() == summary.cone().coordinate()
        && dependency.identity() == summary.cone().identity()
        && dependency.hir_fingerprint() == semantic.hir()
        && dependency.mir_fingerprint() == semantic.mir()
        && dependency.lir_fingerprint() == semantic.lir()
}

pub(super) fn validate_acyclic(
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
) -> DependencyValidationResult<()> {
    let mut complete = BTreeSet::new();
    for start in nodes.keys().copied() {
        if complete.contains(&start) {
            continue;
        }
        let mut visiting = BTreeMap::<ConeIdentity, usize>::new();
        let mut stack = vec![(start, 0_usize)];
        visiting.insert(start, 0);
        while let Some((identity, next_index)) = stack.last_mut() {
            let dependencies = nodes[identity].summary.direct_dependencies();
            let mut next = None;
            while *next_index < dependencies.len() {
                let dependency = dependencies[*next_index].identity();
                *next_index += 1;
                if nodes.contains_key(&dependency) {
                    next = Some(dependency);
                    break;
                }
            }
            let Some(next) = next else {
                let (finished, _) = stack.pop().expect("stack is known non-empty");
                visiting.remove(&finished);
                complete.insert(finished);
                continue;
            };
            if let Some(position) = visiting.get(&next).copied() {
                let mut cycle = stack[position..]
                    .iter()
                    .map(|(identity, _)| nodes[identity].summary.cone().coordinate().clone())
                    .collect::<Vec<_>>();
                cycle.push(nodes[&next].summary.cone().coordinate().clone());
                return Err(ExplicitDependencyValidationError::DependencyCycle { cycle }.into());
            }
            if !complete.contains(&next) {
                visiting.insert(next, stack.len());
                stack.push((next, 0));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_support_closure(
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
) -> DependencyValidationResult<()> {
    let direct = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Direct)
        .map(|(identity, _)| *identity)
        .collect::<BTreeSet<_>>();
    let actual_support = nodes
        .iter()
        .filter(|(_, node)| node.input.role == ExplicitDependencyRole::Support)
        .map(|(identity, _)| *identity)
        .collect::<BTreeSet<_>>();
    let mut reachable = direct.clone();
    let mut pending = direct.iter().copied().collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        for dependency in nodes[&identity].summary.direct_dependencies() {
            let dependency = dependency.identity();
            if nodes.contains_key(&dependency) && reachable.insert(dependency) {
                pending.push(dependency);
            }
        }
    }
    let expected_support = reachable
        .difference(&direct)
        .copied()
        .collect::<BTreeSet<_>>();
    if expected_support != actual_support {
        return Err(ExplicitDependencyValidationError::SupportClosure {
            missing: expected_support
                .difference(&actual_support)
                .map(|identity| nodes[identity].summary.cone().coordinate().clone())
                .collect(),
            unexpected: actual_support
                .difference(&expected_support)
                .map(|identity| nodes[identity].summary.cone().coordinate().clone())
                .collect(),
        }
        .into());
    }
    Ok(())
}

pub(super) fn dependency_first_order(
    nodes: &BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
) -> DependencyValidationResult<Vec<ConeIdentity>> {
    let mut remaining = nodes
        .iter()
        .map(|(identity, node)| {
            let count = node
                .summary
                .direct_dependencies()
                .iter()
                .filter(|dependency| nodes.contains_key(&dependency.identity()))
                .count();
            (*identity, count)
        })
        .collect::<BTreeMap<_, _>>();
    let mut dependents = nodes
        .keys()
        .map(|identity| (*identity, Vec::new()))
        .collect::<BTreeMap<_, Vec<ConeIdentity>>>();
    for (dependent, node) in nodes {
        for dependency in node.summary.direct_dependencies() {
            if nodes.contains_key(&dependency.identity()) {
                dependents
                    .get_mut(&dependency.identity())
                    .expect("validated dependency nodes have complete adjacency")
                    .push(*dependent);
            }
        }
    }
    for identities in dependents.values_mut() {
        identities.sort_by(|left, right| node_order(nodes, *left).cmp(&node_order(nodes, *right)));
    }

    let mut ready = BTreeSet::new();
    for (identity, count) in &remaining {
        if *count == 0 {
            ready.insert(node_order(nodes, *identity));
        }
    }
    let mut output = Vec::with_capacity(nodes.len());
    while let Some(next) = ready.pop_first() {
        output.push(next.identity);
        for dependent in &dependents[&next.identity] {
            let Some(count) = remaining.get_mut(dependent) else {
                return Err(ExplicitDependencyValidationError::DependencyOrderState {
                    identity: *dependent,
                }
                .into());
            };
            let Some(updated) = count.checked_sub(1) else {
                return Err(ExplicitDependencyValidationError::DependencyOrderState {
                    identity: *dependent,
                }
                .into());
            };
            *count = updated;
            if updated == 0 {
                ready.insert(node_order(nodes, *dependent));
            }
        }
    }
    if output.len() != nodes.len() {
        return Err(ExplicitDependencyValidationError::DependencyOrderLength {
            expected: nodes.len(),
            actual: output.len(),
        }
        .into());
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodeOrderKey<'a> {
    group: &'a str,
    name: &'a str,
    version: &'a str,
    identity: ConeIdentity,
}

impl Ord for NodeOrderKey<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.group
            .as_bytes()
            .cmp(other.group.as_bytes())
            .then_with(|| self.name.as_bytes().cmp(other.name.as_bytes()))
            .then_with(|| self.version.as_bytes().cmp(other.version.as_bytes()))
            .then_with(|| self.identity.cmp(&other.identity))
    }
}

impl PartialOrd for NodeOrderKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn node_order<'nodes>(
    nodes: &'nodes BTreeMap<ConeIdentity, ValidatedDependencyNode<'_>>,
    identity: ConeIdentity,
) -> NodeOrderKey<'nodes> {
    let coordinate = nodes[&identity].summary.cone().coordinate();
    NodeOrderKey {
        group: coordinate.group(),
        name: coordinate.name(),
        version: coordinate.version(),
        identity,
    }
}
