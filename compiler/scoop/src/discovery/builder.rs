use super::plans::{
    ManifestDependencyPlanLocator, manifest_dependency_plans, prebuilt_dependency_plans,
};
use super::*;

pub(super) struct DiscoveryBuilder {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    explicit: Vec<PendingExplicitDependency>,
    unlocated: Vec<PendingUnlocatedDependency>,
    context: BuildContext,
}

impl DiscoveryBuilder {
    pub(super) fn new(loaded: LoadedBuildRoot) -> Result<Self, BuildGraphDiscoveryError> {
        let LoadedBuildRoot { root, context } = loaded;
        let root_identity = match &root {
            LoadedRootInput::Manifest(manifest) => manifest.identity(),
            LoadedRootInput::SingleFile(_) => ConeIdentity::SINGLE_FILE,
        };
        let mut builder = Self {
            root: root_identity,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            explicit: Vec::new(),
            unlocated: Vec::new(),
            context,
        };
        match root {
            LoadedRootInput::Manifest(manifest) => {
                builder.insert_node(root_identity, GraphNode::ManifestSource(manifest))?;
                builder.expand_source(root_identity)?;
            }
            LoadedRootInput::SingleFile(source) => {
                builder.insert_node(root_identity, GraphNode::SingleFile(source))?;
                builder.insert_edge(DiscoveredDependencyEdge::new(
                    root_identity,
                    ConeIdentity::CORE,
                    ConeCoordinate::reserved_core(),
                    EdgeOrigin::SyntheticSingleFileCore,
                    None,
                ))?;
            }
        }
        Ok(builder)
    }

    pub(super) fn run(mut self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        loop {
            self.discover_declared_dependencies()?;
            if self.nodes.contains_key(&ConeIdentity::CORE) {
                break;
            }
            let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
                self.context.sysroot.as_path(),
                self.context.target.lir_target_selection(),
            );
            let manifest = crate::locator::load_dependency_manifest(
                &ConeCoordinate::reserved_core(),
                layout.source_root().to_path_buf(),
            )
            .map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?;
            let identity = manifest.identity();
            self.insert_node(identity, GraphNode::ManifestSource(Box::new(manifest)))?;
            self.expand_source(identity)?;
        }
        Ok(DiscoveredBuildGraph {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            context: self.context,
        })
    }

    fn discover_declared_dependencies(&mut self) -> Result<(), BuildGraphDiscoveryError> {
        while let Some(pending) = pop_explicit(&mut self.explicit) {
            let claim = {
                let nodes = &self.nodes;
                let Some(GraphNode::ManifestSource(parent)) = nodes.get(&pending.dependent) else {
                    return Err(BuildGraphDiscoveryError::InternalSourceClaim(
                        pending.dependent,
                    ));
                };
                locate_manifest_dependency(
                    parent,
                    &pending.key,
                    &self.context.artifact_search_roots,
                    self.context.target.lir_target_selection(),
                    self.context.limits.artifact_decode(),
                )
                .map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?
            };
            self.intern_claim(claim)?;
        }

        while let Some(pending) = pop_unlocated(&mut self.unlocated) {
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
            let claim = locate_from_search_roots(
                &pending.coordinate,
                &self.context.artifact_search_roots,
                self.context.target.lir_target_selection(),
                self.context.limits.artifact_decode(),
            )
            .map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?;
            self.intern_claim(LocatedDependencyClaim::Prebuilt(Box::new(claim)))?;
        }

        Ok(())
    }

    fn intern_claim(
        &mut self,
        claim: LocatedDependencyClaim,
    ) -> Result<(), BuildGraphDiscoveryError> {
        match claim {
            LocatedDependencyClaim::Source(source) => {
                let identity = source.manifest().identity();
                if self.intern_source(*source)? {
                    self.expand_source(identity)?;
                }
            }
            LocatedDependencyClaim::Prebuilt(prebuilt) => {
                let identity = prebuilt
                    .coordinate()
                    .identity()
                    .map_err(BuildGraphDiscoveryError::Identity)?;
                if self.intern_prebuilt(*prebuilt)? {
                    self.expand_prebuilt(identity)?;
                }
            }
        }
        Ok(())
    }

    fn intern_source(
        &mut self,
        source: ManifestSourceProjection,
    ) -> Result<bool, BuildGraphDiscoveryError> {
        let identity = source.manifest().identity();
        match self.nodes.get(&identity) {
            None => {
                self.insert_node(
                    identity,
                    GraphNode::ManifestSource(Box::new(source.into_manifest())),
                )?;
                Ok(true)
            }
            Some(GraphNode::ManifestSource(existing)) => {
                if existing.coordinate() != source.manifest().coordinate() {
                    return Err(identity_coordinate_conflict(
                        identity,
                        existing.coordinate(),
                        source.manifest().coordinate(),
                    ));
                }
                if existing.real_root() != source.manifest().real_root() {
                    return Err(BuildGraphDiscoveryError::ConflictingSourceLocator {
                        coordinate: existing.coordinate().clone(),
                        first: existing.real_root().to_path_buf(),
                        second: source.manifest().real_root().to_path_buf(),
                    });
                }
                Ok(false)
            }
            Some(GraphNode::Prebuilt(existing)) => {
                Err(BuildGraphDiscoveryError::ConflictingNodeRepresentation {
                    coordinate: existing.coordinate().clone(),
                })
            }
            Some(GraphNode::SingleFile(_)) => {
                Err(BuildGraphDiscoveryError::ReservedNodeClaim(identity))
            }
        }
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

    fn expand_source(&mut self, identity: ConeIdentity) -> Result<(), BuildGraphDiscoveryError> {
        let plans = {
            let Some(GraphNode::ManifestSource(manifest)) = self.nodes.get(&identity) else {
                return Err(BuildGraphDiscoveryError::InternalSourceClaim(identity));
            };
            manifest_dependency_plans(manifest)?
        };
        if identity != ConeIdentity::CORE
            && !plans
                .iter()
                .any(|plan| plan.edge.dependency() == ConeIdentity::CORE)
        {
            self.insert_edge(DiscoveredDependencyEdge::new(
                identity,
                ConeIdentity::CORE,
                ConeCoordinate::reserved_core(),
                EdgeOrigin::InjectedTrustedCore {
                    dependent: identity,
                },
                None,
            ))?;
        }
        for plan in plans {
            self.insert_edge(plan.edge)?;
            match plan.locator {
                ManifestDependencyPlanLocator::Explicit(key) => {
                    self.explicit.push(PendingExplicitDependency {
                        dependent: identity,
                        coordinate: plan.coordinate,
                        key,
                    });
                }
                ManifestDependencyPlanLocator::SearchRoots => {
                    self.unlocated.push(PendingUnlocatedDependency {
                        coordinate: plan.coordinate,
                    });
                }
            }
        }
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

struct PendingExplicitDependency {
    dependent: ConeIdentity,
    coordinate: ConeCoordinate,
    key: DependencyCoordinateKey,
}

struct PendingUnlocatedDependency {
    coordinate: ConeCoordinate,
}

fn pop_explicit(pending: &mut Vec<PendingExplicitDependency>) -> Option<PendingExplicitDependency> {
    let index = pending
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            compare_coordinates(&left.coordinate, &right.coordinate)
                .then_with(|| left.dependent.cmp(&right.dependent))
                .then_with(|| left.key.cmp(&right.key))
        })?
        .0;
    Some(pending.swap_remove(index))
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
