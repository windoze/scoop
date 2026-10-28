//! Project an exact PM graph into the existing compiler schedule.

use super::*;
use crate::ResolvedBuildGraph;
use crate::discovery::ManifestLocatorKind;
use crate::locator::PrebuiltArtifactProjection;
use crate::{BuildFailure, BuildGraphDiscoveryError, BuildResult, BuildRootInput, LoadedBuildRoot};
use scoop_manifest::{DependencyCoordinateKey, LoadedConeManifest};
use scoop_protocol::package::{PreparedConeV1, PreparedResolutionV1};

impl DiscoveredBuildGraph {
    /// The caller has decoded and checked the package protocol boundary.
    /// This projection does not load manifests, inspect artifacts, or resolve versions.
    pub(crate) fn from_package(
        root: ConeIdentity,
        resolution: PreparedResolutionV1,
        context: BuildContext,
    ) -> Result<Self, BuildGraphDiscoveryError> {
        if resolution.target != context.target.canonical_triple()
            || resolution.purpose != context.purpose
        {
            return Err(error(
                "prepared graph target or purpose differs from the request",
            ));
        }
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
        Ok(Self {
            root,
            nodes,
            edges,
            context,
        })
    }
}

impl ResolvedBuildGraph {
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
) -> Result<(EdgeOrigin, Option<DependencyRecord>), BuildGraphDiscoveryError> {
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

fn error(message: impl std::fmt::Display) -> BuildGraphDiscoveryError {
    BuildGraphDiscoveryError::PreparedGraph(message.to_string())
}
