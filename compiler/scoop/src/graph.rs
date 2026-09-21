use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{
    ConeKind, ConeSourceForm, DependencyRecord, SlibClosureDecodeMeterV1, SlibClosureDecodeUsageV1,
    SlibClosureResourceErrorV1,
};

use crate::discovery::{
    BuildContext, DiscoveredBuildGraph, DiscoveredDependencyEdge, DiscoveredGraphParts, EdgeOrigin,
    GraphNode, compare_coordinates,
};
use crate::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildLimitsProfileV1, DiagnosticsPolicy,
    PairedScoopcLocator, TrustedSysrootRoot,
};

mod diagnostic;

pub use diagnostic::*;

#[derive(Debug)]
pub struct ResolvedBuildGraph {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), ResolvedDependencyEdge>,
    dependency_first: Vec<ConeIdentity>,
    source_inputs: BTreeMap<ConeIdentity, ResolvedDependencyProjection>,
    root_kind: RequestedConeKind,
    target_selection: ValidatedLirTargetSelection,
    context: BuildContext,
    meter: SlibClosureDecodeMeterV1,
}

impl DiscoveredBuildGraph {
    /// Resolves every discovered claim into one validated, acyclic graph and
    /// computes the only canonical dependency-first schedule.
    pub fn resolve(self) -> Result<ResolvedBuildGraph, ResolveBuildGraphError> {
        GraphResolver::new(self.into_parts()).resolve()
    }
}

impl ResolvedBuildGraph {
    pub const fn root_identity(&self) -> ConeIdentity {
        self.root
    }

    pub const fn trusted_core_identity(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }

    pub const fn root_kind(&self) -> RequestedConeKind {
        self.root_kind
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn node_coordinate(&self, identity: ConeIdentity) -> Option<&ConeCoordinate> {
        self.nodes.get(&identity).map(GraphNode::coordinate)
    }

    pub fn node_representation(
        &self,
        identity: ConeIdentity,
    ) -> Option<ResolvedNodeRepresentation> {
        self.nodes.get(&identity).map(resolved_representation)
    }

    pub fn node_kind(&self, identity: ConeIdentity) -> Option<RequestedConeKind> {
        self.nodes.get(&identity).map(node_kind)
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = &ResolvedDependencyEdge> {
        self.edges.values()
    }

    pub fn dependency_first(&self) -> &[ConeIdentity] {
        &self.dependency_first
    }

    pub fn source_dependencies(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ResolvedDependencyProjection> {
        self.source_inputs.get(&identity)
    }

    pub fn artifact_search_roots(&self) -> &[ArtifactSearchRoot] {
        &self.context.artifact_search_roots
    }

    pub const fn cache_root(&self) -> &ArtifactCacheRoot {
        &self.context.cache_root
    }

    pub const fn sysroot(&self) -> &TrustedSysrootRoot {
        &self.context.sysroot
    }

    pub const fn target(&self) -> &scoop_toolchain::ResolvedTargetProfile {
        &self.context.target
    }

    pub const fn compiler(&self) -> &PairedScoopcLocator {
        &self.context.compiler
    }

    pub const fn diagnostics(&self) -> DiagnosticsPolicy {
        self.context.diagnostics
    }

    pub const fn limits(&self) -> BuildLimitsProfileV1 {
        self.context.limits
    }

    pub const fn decode_usage(&self) -> SlibClosureDecodeUsageV1 {
        self.meter.usage()
    }

    pub(crate) fn into_parts(self) -> ResolvedGraphParts {
        ResolvedGraphParts {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            dependency_first: self.dependency_first,
            source_inputs: self.source_inputs,
            root_kind: self.root_kind,
            target_selection: self.target_selection,
            context: self.context,
            meter: self.meter,
        }
    }
}

pub(crate) struct ResolvedGraphParts {
    pub(crate) root: ConeIdentity,
    pub(crate) nodes: BTreeMap<ConeIdentity, GraphNode>,
    pub(crate) edges: BTreeMap<(ConeIdentity, ConeIdentity), ResolvedDependencyEdge>,
    pub(crate) dependency_first: Vec<ConeIdentity>,
    pub(crate) source_inputs: BTreeMap<ConeIdentity, ResolvedDependencyProjection>,
    pub(crate) root_kind: RequestedConeKind,
    pub(crate) target_selection: ValidatedLirTargetSelection,
    pub(crate) context: BuildContext,
    pub(crate) meter: SlibClosureDecodeMeterV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedNodeRepresentation {
    ManifestSource,
    PrebuiltArtifact,
    SingleFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDependencyEdge {
    inner: DiscoveredDependencyEdge,
}

impl ResolvedDependencyEdge {
    pub const fn dependent(&self) -> ConeIdentity {
        self.inner.dependent()
    }

    pub const fn dependency(&self) -> ConeIdentity {
        self.inner.dependency()
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.inner.coordinate()
    }

    pub fn origins(&self) -> &[EdgeOrigin] {
        self.inner.origins()
    }

    pub const fn expected_semantic(&self) -> Option<&DependencyRecord> {
        self.inner.expected_semantic()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDependencyProjection {
    direct: Vec<ConeIdentity>,
    support: Vec<ConeIdentity>,
}

impl ResolvedDependencyProjection {
    pub fn direct(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn support(&self) -> &[ConeIdentity] {
        &self.support
    }
}

struct GraphResolver {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    context: BuildContext,
    meter: SlibClosureDecodeMeterV1,
}

impl GraphResolver {
    fn new(parts: DiscoveredGraphParts) -> Self {
        Self {
            root: parts.root,
            nodes: parts.nodes,
            edges: parts.edges,
            context: parts.context,
            meter: parts.meter,
        }
    }

    fn resolve(mut self) -> Result<ResolvedBuildGraph, ResolveBuildGraphError> {
        let root_kind = validate_nodes(self.root, &self.nodes, &self.edges)?;
        validate_edges(&self.nodes, &self.edges)?;
        validate_reachability(self.root, &self.nodes, &self.edges)?;
        let adjacency = canonical_adjacency(&self.nodes, &self.edges);
        let cycles = find_cycles(&self.nodes, &self.edges, &adjacency)?;
        if !cycles.is_empty() {
            return Err(ResolveBuildGraphError::Cycles(cycles));
        }

        let node_count = u64::try_from(self.nodes.len()).map_err(|_| {
            ResolveBuildGraphError::Resource(SlibClosureResourceErrorV1::Overflow {
                resource: scoop_slib::SlibClosureResourceKindV1::ConeNodes,
            })
        })?;
        let edge_count = u64::try_from(self.edges.len()).map_err(|_| {
            ResolveBuildGraphError::Resource(SlibClosureResourceErrorV1::Overflow {
                resource: scoop_slib::SlibClosureResourceKindV1::DependencyEdges,
            })
        })?;
        self.meter
            .charge_stable_kahn(node_count, edge_count)
            .map_err(ResolveBuildGraphError::Resource)?;
        let dependency_first = stable_kahn(&self.nodes, &adjacency)?;
        let graph_depth = graph_depth(&adjacency, &dependency_first)?;
        self.meter
            .charge_graph(0, 0, graph_depth)
            .map_err(ResolveBuildGraphError::Resource)?;
        let source_count = u64::try_from(
            self.nodes
                .values()
                .filter(|node| !matches!(node, GraphNode::Prebuilt(_)))
                .count(),
        )
        .map_err(|_| {
            ResolveBuildGraphError::Resource(SlibClosureResourceErrorV1::Overflow {
                resource: scoop_slib::SlibClosureResourceKindV1::ConeNodes,
            })
        })?;
        self.meter
            .charge_graph_projections(source_count, node_count, edge_count)
            .map_err(ResolveBuildGraphError::Resource)?;
        let source_inputs = source_dependency_projections(&self.nodes, &adjacency);
        let target_selection = self.context.target.lir_target_selection();
        let edges = self
            .edges
            .into_iter()
            .map(|(key, inner)| (key, ResolvedDependencyEdge { inner }))
            .collect();

        Ok(ResolvedBuildGraph {
            root: self.root,
            nodes: self.nodes,
            edges,
            dependency_first,
            source_inputs,
            root_kind,
            target_selection,
            context: self.context,
            meter: self.meter,
        })
    }
}

fn validate_nodes(
    root: ConeIdentity,
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
) -> Result<RequestedConeKind, ResolveBuildGraphError> {
    let Some(root_node) = nodes.get(&root) else {
        return Err(ResolveBuildGraphError::MissingRoot(root));
    };
    if !nodes.contains_key(&ConeIdentity::CORE) {
        return Err(ResolveBuildGraphError::MissingTrustedCore);
    }

    for (identity, node) in nodes {
        let derived = node
            .coordinate()
            .identity()
            .map_err(ResolveBuildGraphError::Identity)?;
        if derived != *identity {
            return Err(ResolveBuildGraphError::NodeIdentityMismatch {
                key: *identity,
                coordinate: Box::new(node.coordinate().clone()),
                derived,
            });
        }
        validate_reserved_node(*identity, node)?;
        if *identity != root && node_kind(node) == RequestedConeKind::Executable {
            return Err(ResolveBuildGraphError::ExecutableDependency {
                coordinate: node.coordinate().clone(),
            });
        }
    }

    let version_conflicts = multiple_version_conflicts(nodes, edges);
    if !version_conflicts.is_empty() {
        return Err(ResolveBuildGraphError::MultipleVersions(version_conflicts));
    }

    match root_node {
        GraphNode::ManifestSource(_) | GraphNode::SingleFile(_) => Ok(node_kind(root_node)),
        GraphNode::Prebuilt(_) => Err(ResolveBuildGraphError::InvalidRootRepresentation {
            identity: root,
            representation: resolved_representation(root_node),
        }),
    }
}

fn validate_reserved_node(
    identity: ConeIdentity,
    node: &GraphNode,
) -> Result<(), ResolveBuildGraphError> {
    let coordinate = node.coordinate();
    let is_single_coordinate = coordinate == &ConeCoordinate::reserved_single_file();
    match node {
        GraphNode::SingleFile(_) => {
            if identity != ConeIdentity::SINGLE_FILE || !is_single_coordinate {
                return Err(ResolveBuildGraphError::InvalidReservedNode {
                    identity,
                    coordinate: Box::new(coordinate.clone()),
                    representation: resolved_representation(node),
                });
            }
        }
        GraphNode::ManifestSource(_) => {
            if identity == ConeIdentity::SINGLE_FILE || is_single_coordinate {
                return Err(ResolveBuildGraphError::InvalidReservedNode {
                    identity,
                    coordinate: Box::new(coordinate.clone()),
                    representation: resolved_representation(node),
                });
            }
        }
        GraphNode::Prebuilt(prebuilt) => {
            let cone = prebuilt.first_candidate().summary().cone();
            if identity == ConeIdentity::SINGLE_FILE
                || is_single_coordinate
                || cone.kind() != ConeKind::Library
                || cone.source_form() != ConeSourceForm::Manifest
            {
                return Err(ResolveBuildGraphError::InvalidReservedNode {
                    identity,
                    coordinate: Box::new(coordinate.clone()),
                    representation: resolved_representation(node),
                });
            }
        }
    }
    Ok(())
}

fn multiple_version_conflicts(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
) -> Vec<MultipleVersionConflict> {
    let mut groups: BTreeMap<(String, String), Vec<ConeIdentity>> = BTreeMap::new();
    for (identity, node) in nodes {
        groups
            .entry((
                node.coordinate().group().to_owned(),
                node.coordinate().name().to_owned(),
            ))
            .or_default()
            .push(*identity);
    }
    groups
        .into_iter()
        .filter_map(|((group, name), mut identities)| {
            if identities.len() < 2 {
                return None;
            }
            sort_identities(nodes, &mut identities);
            let claims = identities
                .into_iter()
                .map(|identity| {
                    let mut incoming_origins: Vec<_> = edges
                        .values()
                        .filter(|edge| edge.dependency() == identity)
                        .flat_map(|edge| edge.origins().iter().cloned())
                        .collect();
                    incoming_origins.sort();
                    incoming_origins.dedup();
                    VersionedNodeClaim {
                        coordinate: nodes[&identity].coordinate().clone(),
                        identity,
                        incoming_origins,
                    }
                })
                .collect();
            Some(MultipleVersionConflict {
                group,
                name,
                claims,
            })
        })
        .collect()
}

fn validate_edges(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
) -> Result<(), ResolveBuildGraphError> {
    let mut outgoing_counts: BTreeMap<ConeIdentity, usize> = BTreeMap::new();
    let mut core_edges: BTreeSet<ConeIdentity> = BTreeSet::new();
    for ((key_dependent, key_dependency), edge) in edges {
        if *key_dependent != edge.dependent() || *key_dependency != edge.dependency() {
            return Err(ResolveBuildGraphError::EdgeKeyMismatch(Box::new(
                EdgeKeyMismatch {
                    key_dependent: *key_dependent,
                    key_dependency: *key_dependency,
                    edge_dependent: edge.dependent(),
                    edge_dependency: edge.dependency(),
                },
            )));
        }
        let Some(dependent) = nodes.get(&edge.dependent()) else {
            return Err(ResolveBuildGraphError::MissingEdgeEndpoint {
                dependent: edge.dependent(),
                dependency: edge.dependency(),
            });
        };
        let Some(dependency) = nodes.get(&edge.dependency()) else {
            return Err(ResolveBuildGraphError::MissingEdgeEndpoint {
                dependent: edge.dependent(),
                dependency: edge.dependency(),
            });
        };
        if dependency.coordinate() != edge.coordinate() {
            return Err(ResolveBuildGraphError::EdgeCoordinateMismatch {
                dependent: edge.dependent(),
                dependency: edge.dependency(),
                expected: Box::new(dependency.coordinate().clone()),
                actual: Box::new(edge.coordinate().clone()),
            });
        }
        if node_kind(dependency) != RequestedConeKind::Library
            || matches!(dependency, GraphNode::SingleFile(_))
        {
            return Err(ResolveBuildGraphError::ExecutableDependency {
                coordinate: dependency.coordinate().clone(),
            });
        }
        validate_edge_representation(dependent, edge)?;
        if let Some(expected) = edge.expected_semantic()
            && (expected.identity() != edge.dependency()
                || expected.coordinate() != edge.coordinate())
        {
            return Err(ResolveBuildGraphError::DependencyRecordMismatch {
                dependent: edge.dependent(),
                dependency: edge.dependency(),
            });
        }
        *outgoing_counts.entry(edge.dependent()).or_default() += 1;
        if edge.dependency() == ConeIdentity::CORE {
            core_edges.insert(edge.dependent());
        }
    }

    for (identity, node) in nodes {
        if *identity == ConeIdentity::CORE {
            continue;
        }
        if !core_edges.contains(identity) {
            return Err(ResolveBuildGraphError::MissingDirectCore {
                coordinate: node.coordinate().clone(),
            });
        }
        if matches!(node, GraphNode::SingleFile(_))
            && outgoing_counts.get(identity).copied().unwrap_or_default() != 1
        {
            return Err(ResolveBuildGraphError::InvalidSingleFileGraph);
        }
    }
    Ok(())
}

fn validate_edge_representation(
    dependent: &GraphNode,
    edge: &DiscoveredDependencyEdge,
) -> Result<(), ResolveBuildGraphError> {
    let to_core = edge.dependency() == ConeIdentity::CORE;
    let valid = match dependent {
        GraphNode::ManifestSource(_) => {
            edge.expected_semantic().is_none()
                && edge.origins().iter().all(|origin| {
                    matches!(
                        (to_core, origin),
                        (true, EdgeOrigin::InjectedTrustedCore { .. })
                            | (false, EdgeOrigin::ManifestDeclaration { .. })
                    )
                })
        }
        GraphNode::Prebuilt(_) => {
            edge.expected_semantic().is_some()
                && edge
                    .origins()
                    .iter()
                    .all(|origin| matches!(origin, EdgeOrigin::ArtifactDependencyRecord { .. }))
        }
        GraphNode::SingleFile(_) => {
            to_core
                && edge.expected_semantic().is_none()
                && edge
                    .origins()
                    .iter()
                    .all(|origin| matches!(origin, EdgeOrigin::SyntheticSingleFileCore))
        }
    };
    if valid && !edge.origins().is_empty() {
        Ok(())
    } else {
        Err(ResolveBuildGraphError::InvalidEdgeOrigin {
            dependent: edge.dependent(),
            dependency: edge.dependency(),
        })
    }
}

fn validate_reachability(
    root: ConeIdentity,
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
) -> Result<(), ResolveBuildGraphError> {
    let adjacency = canonical_adjacency(nodes, edges);
    let mut reached = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !reached.insert(node) {
            continue;
        }
        if let Some(dependencies) = adjacency.get(&node) {
            pending.extend(dependencies.iter().rev().copied());
        }
    }
    if reached.len() == nodes.len() {
        return Ok(());
    }
    let mut unreachable: Vec<_> = nodes
        .keys()
        .filter(|identity| !reached.contains(identity))
        .copied()
        .collect();
    sort_identities(nodes, &mut unreachable);
    Err(ResolveBuildGraphError::UnreachableNodes(
        unreachable
            .into_iter()
            .map(|identity| nodes[&identity].coordinate().clone())
            .collect(),
    ))
}

fn canonical_adjacency(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
) -> BTreeMap<ConeIdentity, Vec<ConeIdentity>> {
    let mut adjacency: BTreeMap<ConeIdentity, Vec<ConeIdentity>> = nodes
        .keys()
        .map(|identity| (*identity, Vec::new()))
        .collect();
    for edge in edges.values() {
        adjacency
            .entry(edge.dependent())
            .or_default()
            .push(edge.dependency());
    }
    for dependencies in adjacency.values_mut() {
        sort_identities(nodes, dependencies);
    }
    adjacency
}

fn find_cycles(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
) -> Result<Vec<ResolvedCycle>, ResolveBuildGraphError> {
    let mut identities: Vec<_> = nodes.keys().copied().collect();
    sort_identities(nodes, &mut identities);
    let mut visited = BTreeSet::new();
    let mut finish = Vec::with_capacity(nodes.len());
    for identity in identities.iter().copied() {
        if !visited.contains(&identity) {
            append_finishing_order(identity, adjacency, &mut visited, &mut finish);
        }
    }

    let mut reverse: BTreeMap<ConeIdentity, Vec<ConeIdentity>> = nodes
        .keys()
        .map(|identity| (*identity, Vec::new()))
        .collect();
    for (dependent, dependencies) in adjacency {
        for dependency in dependencies {
            reverse.entry(*dependency).or_default().push(*dependent);
        }
    }
    for dependents in reverse.values_mut() {
        sort_identities(nodes, dependents);
    }

    let mut assigned = BTreeSet::new();
    let mut cyclic_components = Vec::new();
    for start in finish.into_iter().rev() {
        if assigned.contains(&start) {
            continue;
        }
        let mut component = Vec::new();
        let mut pending = vec![start];
        assigned.insert(start);
        while let Some(node) = pending.pop() {
            component.push(node);
            if let Some(dependents) = reverse.get(&node) {
                for dependent in dependents.iter().rev().copied() {
                    if assigned.insert(dependent) {
                        pending.push(dependent);
                    }
                }
            }
        }
        sort_identities(nodes, &mut component);
        let cyclic = component.len() > 1
            || adjacency
                .get(&component[0])
                .is_some_and(|dependencies| dependencies.contains(&component[0]));
        if cyclic {
            cyclic_components.push(component);
        }
    }
    cyclic_components.sort_by(|left, right| compare_node_ids(nodes, left[0], right[0]));
    cyclic_components
        .into_iter()
        .map(|component| build_cycle(nodes, edges, adjacency, &component))
        .collect()
}

fn append_finishing_order(
    start: ConeIdentity,
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
    visited: &mut BTreeSet<ConeIdentity>,
    finish: &mut Vec<ConeIdentity>,
) {
    visited.insert(start);
    let mut stack = vec![(start, 0_usize)];
    while let Some((node, next_index)) = stack.last_mut() {
        let dependencies = &adjacency[node];
        if *next_index < dependencies.len() {
            let dependency = dependencies[*next_index];
            *next_index += 1;
            if visited.insert(dependency) {
                stack.push((dependency, 0));
            }
        } else {
            let finished = *node;
            stack.pop();
            finish.push(finished);
        }
    }
}

fn build_cycle(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
    component: &[ConeIdentity],
) -> Result<ResolvedCycle, ResolveBuildGraphError> {
    let members: BTreeSet<_> = component.iter().copied().collect();
    let start = component[0];
    let mut path = vec![start];
    let mut on_path = BTreeSet::from([start]);
    let mut stack = vec![(start, 0_usize)];
    while let Some((node, next_index)) = stack.last_mut() {
        let dependencies = &adjacency[node];
        let mut next = None;
        while *next_index < dependencies.len() {
            let candidate = dependencies[*next_index];
            *next_index += 1;
            if members.contains(&candidate) {
                next = Some(candidate);
                break;
            }
        }
        let Some(dependency) = next else {
            let removed = *node;
            stack.pop();
            path.pop();
            on_path.remove(&removed);
            continue;
        };
        if dependency == start {
            path.push(start);
            return cycle_from_path(nodes, edges, &path);
        }
        if on_path.insert(dependency) {
            path.push(dependency);
            stack.push((dependency, 0));
        }
    }
    Err(ResolveBuildGraphError::InternalCyclePath(start))
}

fn cycle_from_path(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    edges: &BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    path: &[ConeIdentity],
) -> Result<ResolvedCycle, ResolveBuildGraphError> {
    let mut steps = Vec::with_capacity(path.len().saturating_sub(1));
    for pair in path.windows(2) {
        let Some(edge) = edges.get(&(pair[0], pair[1])) else {
            return Err(ResolveBuildGraphError::InternalCycleEdge {
                dependent: pair[0],
                dependency: pair[1],
            });
        };
        steps.push(CycleStep {
            dependent: pair[0],
            dependent_coordinate: nodes[&pair[0]].coordinate().clone(),
            dependency: pair[1],
            dependency_coordinate: nodes[&pair[1]].coordinate().clone(),
            origins: edge.origins().to_vec(),
        });
    }
    Ok(ResolvedCycle { steps })
}

fn stable_kahn(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
) -> Result<Vec<ConeIdentity>, ResolveBuildGraphError> {
    let mut remaining: BTreeMap<_, _> = adjacency
        .iter()
        .map(|(identity, dependencies)| (*identity, dependencies.len()))
        .collect();
    let mut dependents: BTreeMap<ConeIdentity, Vec<ConeIdentity>> = nodes
        .keys()
        .map(|identity| (*identity, Vec::new()))
        .collect();
    for (dependent, dependencies) in adjacency {
        for dependency in dependencies {
            dependents.entry(*dependency).or_default().push(*dependent);
        }
    }
    for values in dependents.values_mut() {
        sort_identities(nodes, values);
    }

    let mut ready = BTreeSet::new();
    for (identity, count) in &remaining {
        if *count == 0 {
            ready.insert(NodeOrderKey::new(nodes, *identity));
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(nodes.len())
        .map_err(|_| ResolveBuildGraphError::Allocation {
            purpose: "dependency-first order",
            elements: nodes.len(),
        })?;
    while let Some(next) = ready.pop_first() {
        let identity = next.identity;
        output.push(identity);
        for dependent in &dependents[&identity] {
            let Some(count) = remaining.get_mut(dependent) else {
                return Err(ResolveBuildGraphError::InternalTopologicalState(*dependent));
            };
            let Some(updated) = count.checked_sub(1) else {
                return Err(ResolveBuildGraphError::InternalTopologicalState(*dependent));
            };
            *count = updated;
            if updated == 0 {
                ready.insert(NodeOrderKey::new(nodes, *dependent));
            }
        }
    }
    if output.len() != nodes.len() {
        return Err(ResolveBuildGraphError::InternalTopologicalLength {
            expected: nodes.len(),
            actual: output.len(),
        });
    }
    Ok(output)
}

fn graph_depth(
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
    dependency_first: &[ConeIdentity],
) -> Result<u64, ResolveBuildGraphError> {
    let mut depths = BTreeMap::new();
    let mut maximum = 0_u64;
    for identity in dependency_first {
        let dependency_depth = adjacency[identity]
            .iter()
            .filter_map(|dependency| depths.get(dependency))
            .copied()
            .max()
            .unwrap_or(0_u64);
        let depth = dependency_depth
            .checked_add(1)
            .ok_or(ResolveBuildGraphError::Resource(
                SlibClosureResourceErrorV1::Overflow {
                    resource: scoop_slib::SlibClosureResourceKindV1::GraphDepth,
                },
            ))?;
        depths.insert(*identity, depth);
        maximum = maximum.max(depth);
    }
    Ok(maximum)
}

fn source_dependency_projections(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    adjacency: &BTreeMap<ConeIdentity, Vec<ConeIdentity>>,
) -> BTreeMap<ConeIdentity, ResolvedDependencyProjection> {
    nodes
        .iter()
        .filter(|(_, node)| !matches!(node, GraphNode::Prebuilt(_)))
        .map(|(identity, _)| {
            let direct: Vec<_> = adjacency[identity]
                .iter()
                .copied()
                .filter(|dependency| *dependency != ConeIdentity::CORE)
                .collect();
            let mut reachable: BTreeSet<_> = direct.iter().copied().collect();
            let mut pending = direct.clone();
            while let Some(node) = pending.pop() {
                for dependency in adjacency[&node].iter().rev().copied() {
                    if reachable.insert(dependency) {
                        pending.push(dependency);
                    }
                }
            }
            reachable.remove(&ConeIdentity::CORE);
            reachable.remove(identity);
            for dependency in &direct {
                reachable.remove(dependency);
            }
            let mut support: Vec<_> = reachable.into_iter().collect();
            sort_identities(nodes, &mut support);
            (*identity, ResolvedDependencyProjection { direct, support })
        })
        .collect()
}

fn node_kind(node: &GraphNode) -> RequestedConeKind {
    match node {
        GraphNode::ManifestSource(manifest) => manifest.parsed().semantic().requested_kind(),
        GraphNode::Prebuilt(prebuilt) => match prebuilt.first_candidate().summary().cone().kind() {
            ConeKind::Library => RequestedConeKind::Library,
            ConeKind::Executable => RequestedConeKind::Executable,
        },
        GraphNode::SingleFile(_) => RequestedConeKind::Executable,
    }
}

fn resolved_representation(node: &GraphNode) -> ResolvedNodeRepresentation {
    match node {
        GraphNode::ManifestSource(_) => ResolvedNodeRepresentation::ManifestSource,
        GraphNode::Prebuilt(_) => ResolvedNodeRepresentation::PrebuiltArtifact,
        GraphNode::SingleFile(_) => ResolvedNodeRepresentation::SingleFile,
    }
}

fn sort_identities(nodes: &BTreeMap<ConeIdentity, GraphNode>, identities: &mut [ConeIdentity]) {
    identities.sort_by(|left, right| compare_node_ids(nodes, *left, *right));
}

fn compare_node_ids(
    nodes: &BTreeMap<ConeIdentity, GraphNode>,
    left: ConeIdentity,
    right: ConeIdentity,
) -> Ordering {
    compare_coordinates(nodes[&left].coordinate(), nodes[&right].coordinate())
        .then_with(|| left.cmp(&right))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodeOrderKey<'a> {
    group: &'a str,
    name: &'a str,
    version: &'a str,
    identity: ConeIdentity,
}

impl<'a> NodeOrderKey<'a> {
    fn new(nodes: &'a BTreeMap<ConeIdentity, GraphNode>, identity: ConeIdentity) -> Self {
        let coordinate = nodes[&identity].coordinate();
        Self {
            group: coordinate.group(),
            name: coordinate.name(),
            version: coordinate.version(),
            identity,
        }
    }
}

impl Ord for NodeOrderKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.group
            .as_bytes()
            .cmp(other.group.as_bytes())
            .then_with(|| self.name.as_bytes().cmp(other.name.as_bytes()))
            .then_with(|| self.version.as_bytes().cmp(other.version.as_bytes()))
            .then_with(|| self.identity.cmp(&other.identity))
    }
}

impl PartialOrd for NodeOrderKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests;
