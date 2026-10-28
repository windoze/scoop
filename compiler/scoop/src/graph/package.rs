//! Project an exact PM graph into the existing compiler schedule.

use super::*;
use crate::discovery::ManifestLocatorKind;
use crate::locator::PrebuiltArtifactProjection;
use crate::{BuildFailure, BuildFailurePhase, BuildResult, BuildRootInput, LoadedBuildRoot};
use scoop_manifest::{DependencyCoordinateKey, LoadedConeManifest};
use scoop_protocol::package::{PreparedConeV1, PreparedResolutionV1};

impl ResolvedBuildGraph {
    /// The caller has decoded and checked the package protocol boundary.
    /// This projection does not load manifests, inspect artifacts, or resolve versions.
    pub fn from_package(
        resolution: PreparedResolutionV1,
        context: BuildContext,
    ) -> BuildResult<Self> {
        if resolution.target != context.target.canonical_triple()
            || resolution.purpose != context.purpose
        {
            return Err(error(
                "prepared graph target or purpose differs from the request",
            ));
        }
        let root = resolution.coordinate.identity().map_err(error)?;
        let mut nodes = BTreeMap::new();
        let mut pending = Vec::new();
        for node in resolution.nodes {
            let identity = node.cone.coordinate().identity().map_err(error)?;
            let graph_node = match node.cone {
                PreparedConeV1::Source { config } => {
                    let manifest = LoadedConeManifest::from_expanded(
                        config.root.to_path_buf().map_err(error)?,
                        config.manifest_path.to_path_buf().map_err(error)?,
                        config.manifest_text,
                        config.manifest,
                        config.metadata,
                    )
                    .map_err(error)?;
                    GraphNode::ManifestSource(Box::new(manifest))
                }
                PreparedConeV1::Prebuilt { artifact, summary } => {
                    GraphNode::Prebuilt(Box::new(PrebuiltArtifactProjection::from_prepared(
                        artifact.to_path_buf().map_err(error)?,
                        *summary,
                    )))
                }
            };
            pending.push((identity, node.dependencies, node.test_dependencies));
            nodes.insert(identity, graph_node);
        }
        let root_node = nodes
            .get(&root)
            .ok_or_else(|| error("prepared root is absent"))?;
        if !matches!(root_node, GraphNode::ManifestSource(_)) {
            return Err(error("prepared root must be a source Cone"));
        }
        let root_kind = match context.purpose {
            scoop_protocol::CompilationPurpose::Production => node_kind(root_node),
            scoop_protocol::CompilationPurpose::Test => RequestedConeKind::Executable,
        };
        let mut edges = BTreeMap::new();
        for (dependent, production, tests) in pending {
            let node = &nodes[&dependent];
            for coordinate in production.into_iter().chain(tests) {
                let dependency = coordinate.identity().map_err(error)?;
                let (origin, expected) = edge_origin(dependent, node, &coordinate)?;
                edges.insert(
                    (dependent, dependency),
                    DiscoveredDependencyEdge::new(
                        dependent, dependency, coordinate, origin, expected,
                    ),
                );
            }
        }
        validate_reachability(root, &nodes, &edges).map_err(BuildFailure::classified)?;
        let adjacency = canonical_adjacency(&nodes, &edges);
        let dependency_first = stable_kahn(&nodes, &adjacency).map_err(BuildFailure::classified)?;
        let source_inputs = source_dependency_projections(&nodes, &adjacency);
        Ok(Self {
            root,
            nodes,
            edges: edges
                .into_iter()
                .map(|(key, inner)| (key, ResolvedDependencyEdge { inner }))
                .collect(),
            dependency_first,
            source_inputs,
            root_kind,
            target_selection: context.target.lir_target_selection(),
            context,
        })
    }

    pub fn load(root: BuildRootInput, context: BuildContext) -> BuildResult<Self> {
        LoadedBuildRoot::from_input(root, context)
            .map_err(BuildFailure::root)?
            .discover()
            .map_err(BuildFailure::discovery)?
            .resolve()
            .map_err(BuildFailure::classified)
    }
}

fn edge_origin(
    dependent: ConeIdentity,
    node: &GraphNode,
    coordinate: &ConeCoordinate,
) -> BuildResult<(EdgeOrigin, Option<DependencyRecord>)> {
    match node {
        GraphNode::ManifestSource(manifest) => {
            let key = DependencyCoordinateKey::from_coordinate(coordinate);
            let span = manifest
                .parsed()
                .diagnostic_spans()
                .dependency(&key)
                .map(|spans| spans.declaration().range())
                .or_else(|| {
                    manifest
                        .parsed()
                        .tests()
                        .dependency_iter()
                        .find(|(candidate, _)| *candidate == &key)
                        .map(|(_, dependency)| dependency.spans().declaration().range())
                });
            let origin = match span {
                Some(span) => EdgeOrigin::ManifestDeclaration {
                    manifest: manifest.manifest_path().to_owned(),
                    span,
                    locator_kind: ManifestLocatorKind::SearchRoots,
                },
                None if coordinate == &ConeCoordinate::reserved_core() => {
                    EdgeOrigin::InjectedTrustedCore { dependent }
                }
                None => {
                    return Err(error(format!(
                        "prepared dependency {coordinate} has no declaration"
                    )));
                }
            };
            Ok((origin, None))
        }
        GraphNode::Prebuilt(prebuilt) => {
            let artifact = prebuilt.first_candidate();
            let (record_index, record) = artifact
                .summary()
                .direct_dependencies()
                .iter()
                .enumerate()
                .find(|(_, record)| record.coordinate() == coordinate)
                .ok_or_else(|| {
                    error(format!(
                        "prepared artifact has no dependency record for {coordinate}"
                    ))
                })?;
            Ok((
                EdgeOrigin::ArtifactDependencyRecord {
                    artifact: artifact.resolved_path().to_owned(),
                    record_index,
                },
                Some(record.clone()),
            ))
        }
        GraphNode::SingleFile(_) => Err(error("package graph contains a single-file node")),
    }
}

fn error(message: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool(
        "SCOOP_PACKAGE_GRAPH",
        BuildFailurePhase::GraphIdentity,
        message,
    )
}
