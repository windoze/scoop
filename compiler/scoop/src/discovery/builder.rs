use super::plans::prebuilt_dependency_plans;
use super::*;

pub(super) struct DiscoveryBuilder {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    unlocated: Vec<PendingUnlocatedDependency>,
    context: BuildContext,
}

impl DiscoveryBuilder {
    pub(super) fn new(
        source: SingleFileLocator,
        dependencies: Vec<scoop_manifest::SingleFileDependency>,
        context: BuildContext,
    ) -> Result<Self, BuildGraphDiscoveryError> {
        let root = ConeIdentity::SINGLE_FILE;
        let mut builder = Self {
            root,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            unlocated: Vec::new(),
            context,
        };
        for dependency in dependencies {
            let coordinate = dependency.coordinate().clone();
            let identity = coordinate
                .identity()
                .map_err(BuildGraphDiscoveryError::Identity)?;
            builder.insert_edge(DiscoveredDependencyEdge::new(
                root,
                identity,
                coordinate.clone(),
                EdgeOrigin::SingleFile {
                    source: source.display_path().to_owned(),
                    span: dependency.span(),
                },
                None,
            ))?;
            if identity != ConeIdentity::CORE {
                builder
                    .unlocated
                    .push(PendingUnlocatedDependency { coordinate });
            }
        }
        builder.insert_node(root, GraphNode::SingleFile(source))?;
        Ok(builder)
    }

    pub(super) fn run(mut self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        loop {
            self.discover_declared_dependencies()?;
            if self.nodes.contains_key(&ConeIdentity::CORE) {
                break;
            }
            let claim = crate::locator::locate_default_core(
                self.context.sysroot.as_path(),
                self.context.target.lir_target_selection(),
                &self.context.cache_root,
            )
            .map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?;
            self.intern_claim(claim)?;
        }
        Ok(DiscoveredBuildGraph {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            context: self.context,
        })
    }

    fn discover_declared_dependencies(&mut self) -> Result<(), BuildGraphDiscoveryError> {
        loop {
            let Some(pending) = pop_unlocated(&mut self.unlocated) else {
                return Ok(());
            };
            let identity = pending
                .coordinate
                .identity()
                .map_err(BuildGraphDiscoveryError::Identity)?;
            if let Some(existing) = self.nodes.get(&identity) {
                if existing.coordinate() != &pending.coordinate {
                    return Err(identity_coordinate_conflict(
                        identity,
                        existing.coordinate(),
                        &pending.coordinate,
                    ));
                }
                continue;
            }
            let claim = match locate_build_dependency(
                &pending.coordinate,
                &self.context.artifact_search_roots,
                self.context.sysroot.as_path(),
                self.context.target.lir_target_selection(),
                &self.context.cache_root,
            ) {
                // A version-only core edge can use the graph's eventual core.
                // Defer the default slot until other declarations are expanded.
                Err(DependencyLocatorError::ArtifactNotFound { .. })
                    if identity == ConeIdentity::CORE =>
                {
                    continue;
                }
                result => {
                    result.map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?
                }
            };
            self.intern_claim(claim)?;
        }
    }

    fn intern_claim(
        &mut self,
        prebuilt: PrebuiltArtifactProjection,
    ) -> Result<(), BuildGraphDiscoveryError> {
        let identity = prebuilt
            .coordinate()
            .identity()
            .map_err(BuildGraphDiscoveryError::Identity)?;
        if self.intern_prebuilt(prebuilt)? {
            self.expand_prebuilt(identity)?;
        }
        Ok(())
    }

    fn intern_prebuilt(
        &mut self,
        prebuilt: PrebuiltArtifactProjection,
    ) -> Result<bool, BuildGraphDiscoveryError> {
        let identity = prebuilt
            .coordinate()
            .identity()
            .map_err(BuildGraphDiscoveryError::Identity)?;
        match self.nodes.get_mut(&identity) {
            None => {
                self.nodes
                    .insert(identity, GraphNode::Prebuilt(Box::new(prebuilt)));
                Ok(true)
            }
            Some(GraphNode::Prebuilt(existing)) => {
                if existing.coordinate() != prebuilt.coordinate() {
                    return Err(identity_coordinate_conflict(
                        identity,
                        existing.coordinate(),
                        prebuilt.coordinate(),
                    ));
                }
                if existing.artifact_fingerprint() != prebuilt.artifact_fingerprint() {
                    return Err(BuildGraphDiscoveryError::AmbiguousArtifact {
                        coordinate: Box::new(existing.coordinate().clone()),
                        first: existing.artifact_fingerprint(),
                        second: prebuilt.artifact_fingerprint(),
                    });
                }
                existing.merge_same_artifact(prebuilt);
                Ok(false)
            }
            Some(GraphNode::ManifestSource(existing)) => {
                Err(BuildGraphDiscoveryError::ConflictingNodeRepresentation {
                    coordinate: existing.coordinate().clone(),
                })
            }
            Some(GraphNode::SingleFile(_)) => {
                Err(BuildGraphDiscoveryError::ReservedNodeClaim(identity))
            }
        }
    }

    fn insert_node(
        &mut self,
        identity: ConeIdentity,
        node: GraphNode,
    ) -> Result<(), BuildGraphDiscoveryError> {
        self.nodes.insert(identity, node);
        Ok(())
    }

    fn insert_edge(
        &mut self,
        edge: DiscoveredDependencyEdge,
    ) -> Result<(), BuildGraphDiscoveryError> {
        let key = (edge.dependent, edge.dependency);
        if let Some(existing) = self.edges.get_mut(&key) {
            if existing.coordinate != edge.coordinate
                || existing.expected_semantic != edge.expected_semantic
            {
                return Err(BuildGraphDiscoveryError::ConflictingDependencyEdge {
                    dependent: edge.dependent,
                    dependency: edge.dependency,
                });
            }
            for origin in edge.origins {
                if !existing.origins.contains(&origin) {
                    existing.origins.push(origin);
                }
            }
            existing.origins.sort();
            return Ok(());
        }

        self.edges.insert(key, edge);
        Ok(())
    }

    fn expand_prebuilt(&mut self, identity: ConeIdentity) -> Result<(), BuildGraphDiscoveryError> {
        let plans = {
            let Some(GraphNode::Prebuilt(prebuilt)) = self.nodes.get(&identity) else {
                return Err(BuildGraphDiscoveryError::InternalPrebuiltClaim(identity));
            };
            prebuilt_dependency_plans(identity, prebuilt)
        };
        for plan in plans {
            self.insert_edge(plan.edge)?;
            if plan.coordinate != ConeCoordinate::reserved_core() {
                self.unlocated.push(PendingUnlocatedDependency {
                    coordinate: plan.coordinate,
                });
            }
        }
        Ok(())
    }
}

struct PendingUnlocatedDependency {
    coordinate: ConeCoordinate,
}

fn pop_unlocated(
    pending: &mut Vec<PendingUnlocatedDependency>,
) -> Option<PendingUnlocatedDependency> {
    let index = pending
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| compare_coordinates(&left.coordinate, &right.coordinate))?
        .0;
    Some(pending.swap_remove(index))
}

fn identity_coordinate_conflict(
    identity: ConeIdentity,
    first: &ConeCoordinate,
    second: &ConeCoordinate,
) -> BuildGraphDiscoveryError {
    BuildGraphDiscoveryError::IdentityCoordinateConflict {
        identity,
        first: Box::new(first.clone()),
        second: Box::new(second.clone()),
    }
}
