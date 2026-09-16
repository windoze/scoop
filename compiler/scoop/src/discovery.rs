use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::PathBuf;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::{
    DependencyCoordinateKey, DependencyLocator, LoadedConeManifest, ManifestRootError,
    ManifestRootLocator, SingleFileLocator, load_cone_manifest, load_trusted_core_manifest,
};
use scoop_slib::{
    DependencyRecord, SlibClosureDecodeMeterV1, SlibClosureDecodeUsageV1,
    SlibClosureResourceErrorV1,
};
use scoop_wire::HashError;

use crate::locator::{
    DependencyLocatorError, LocatedDependencyClaim, ManifestSourceProjection,
    PrebuiltArtifactProjection, locate_from_search_roots, locate_manifest_dependency,
};
use crate::request::{BuildGraphRequestParts, BuildRootInputKind};
use crate::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildGraphRequest, BuildLimitsProfileV1,
    DiagnosticsPolicy, PairedScoopcLocator, TrustedSysrootRoot,
};

#[derive(Debug)]
pub struct LoadedBuildRoot {
    root: LoadedRootInput,
    trusted_core: LoadedConeManifest,
    context: BuildContext,
    meter: SlibClosureDecodeMeterV1,
}

#[derive(Debug)]
enum LoadedRootInput {
    Manifest(Box<LoadedConeManifest>),
    SingleFile(SingleFileLocator),
}

#[derive(Debug)]
pub(crate) struct BuildContext {
    pub(crate) artifact_search_roots: Vec<ArtifactSearchRoot>,
    pub(crate) cache_root: ArtifactCacheRoot,
    pub(crate) sysroot: TrustedSysrootRoot,
    pub(crate) target: scoop_toolchain::ResolvedTargetProfile,
    pub(crate) compiler: PairedScoopcLocator,
    pub(crate) diagnostics: DiagnosticsPolicy,
    pub(crate) limits: BuildLimitsProfileV1,
}

impl BuildGraphRequest {
    /// Loads only the selected root operand and the trusted core source slot.
    /// No dependency locator or current-Cone source tree is traversed here.
    pub fn load_root(self) -> Result<LoadedBuildRoot, LoadBuildRootError> {
        let BuildGraphRequestParts {
            root,
            artifact_search_roots,
            cache_root,
            sysroot,
            target,
            compiler,
            diagnostics,
            limits,
        } = self.into_parts();
        let mut meter = SlibClosureDecodeMeterV1::new(limits.slib_closure().limits());
        let search_root_count = u64::try_from(artifact_search_roots.len()).map_err(|_| {
            LoadBuildRootError::Resource(SlibClosureResourceErrorV1::Overflow {
                resource: scoop_slib::SlibClosureResourceKindV1::ArtifactSearchRoots,
            })
        })?;
        meter
            .charge_search_roots(search_root_count)
            .map_err(LoadBuildRootError::Resource)?;

        let root = match root.into_kind() {
            BuildRootInputKind::ManifestCone(locator) => LoadedRootInput::Manifest(Box::new(
                load_cone_manifest(&locator).map_err(LoadBuildRootError::RootManifest)?,
            )),
            BuildRootInputKind::SingleFile(locator) => LoadedRootInput::SingleFile(locator),
        };
        let core_layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
            sysroot.as_path(),
            target.lir_target_selection(),
        );
        let core_locator = ManifestRootLocator::cone_directory(core_layout.source_root());
        let trusted_core = load_trusted_core_manifest(&core_locator)
            .map_err(LoadBuildRootError::TrustedCoreManifest)?;
        Ok(LoadedBuildRoot {
            root,
            trusted_core,
            context: BuildContext {
                artifact_search_roots,
                cache_root,
                sysroot,
                target,
                compiler,
                diagnostics,
                limits,
            },
            meter,
        })
    }
}

impl LoadedBuildRoot {
    pub fn root_identity(&self) -> ConeIdentity {
        match &self.root {
            LoadedRootInput::Manifest(manifest) => manifest.identity(),
            LoadedRootInput::SingleFile(_) => ConeIdentity::SINGLE_FILE,
        }
    }

    pub const fn trusted_core_manifest(&self) -> &LoadedConeManifest {
        &self.trusted_core
    }

    /// Discovers exact source and prebuilt claims without constructing any
    /// Compile or Link artifact authority.
    pub fn discover(self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        DiscoveryBuilder::new(self)?.run()
    }
}

#[derive(Debug)]
pub struct DiscoveredBuildGraph {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    context: BuildContext,
    meter: SlibClosureDecodeMeterV1,
}

impl DiscoveredBuildGraph {
    pub const fn root_identity(&self) -> ConeIdentity {
        self.root
    }

    pub const fn trusted_core_identity(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn contains_node(&self, identity: ConeIdentity) -> bool {
        self.nodes.contains_key(&identity)
    }

    pub fn node_coordinate(&self, identity: ConeIdentity) -> Option<&ConeCoordinate> {
        self.nodes.get(&identity).map(GraphNode::coordinate)
    }

    pub fn node_representation(
        &self,
        identity: ConeIdentity,
    ) -> Option<DiscoveredNodeRepresentation> {
        self.nodes.get(&identity).map(GraphNode::representation)
    }

    pub fn prebuilt_candidate_count(&self, identity: ConeIdentity) -> Option<usize> {
        match self.nodes.get(&identity) {
            Some(GraphNode::Prebuilt(prebuilt)) => Some(prebuilt.candidate_count()),
            Some(
                GraphNode::ManifestSource(_) | GraphNode::TrustedCore(_) | GraphNode::SingleFile(_),
            )
            | None => None,
        }
    }

    pub fn edges(&self) -> impl ExactSizeIterator<Item = &DiscoveredDependencyEdge> {
        self.edges.values()
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

    pub(crate) fn into_parts(self) -> DiscoveredGraphParts {
        DiscoveredGraphParts {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            context: self.context,
            meter: self.meter,
        }
    }
}

pub(crate) struct DiscoveredGraphParts {
    pub(crate) root: ConeIdentity,
    pub(crate) nodes: BTreeMap<ConeIdentity, GraphNode>,
    pub(crate) edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    pub(crate) context: BuildContext,
    pub(crate) meter: SlibClosureDecodeMeterV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveredNodeRepresentation {
    ManifestSource,
    PrebuiltArtifact,
    TrustedCore,
    SingleFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDependencyEdge {
    dependent: ConeIdentity,
    dependency: ConeIdentity,
    coordinate: ConeCoordinate,
    origins: Vec<EdgeOrigin>,
    expected_semantic: Option<DependencyRecord>,
}

impl DiscoveredDependencyEdge {
    pub(crate) fn new(
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        coordinate: ConeCoordinate,
        origin: EdgeOrigin,
        expected_semantic: Option<DependencyRecord>,
    ) -> Self {
        Self {
            dependent,
            dependency,
            coordinate,
            origins: vec![origin],
            expected_semantic,
        }
    }

    pub const fn dependent(&self) -> ConeIdentity {
        self.dependent
    }

    pub const fn dependency(&self) -> ConeIdentity {
        self.dependency
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub fn origins(&self) -> &[EdgeOrigin] {
        &self.origins
    }

    pub const fn expected_semantic(&self) -> Option<&DependencyRecord> {
        self.expected_semantic.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EdgeOrigin {
    ManifestDeclaration {
        manifest: PathBuf,
        span: Range<usize>,
        locator_kind: ManifestLocatorKind,
    },
    ArtifactDependencyRecord {
        artifact: PathBuf,
        record_index: usize,
    },
    InjectedTrustedCore {
        dependent: ConeIdentity,
    },
    SyntheticSingleFileCore,
}

impl Ord for EdgeOrigin {
    fn cmp(&self, other: &Self) -> Ordering {
        edge_origin_rank(self)
            .cmp(&edge_origin_rank(other))
            .then_with(|| match (self, other) {
                (
                    Self::ManifestDeclaration {
                        manifest: left_manifest,
                        span: left_span,
                        locator_kind: left_kind,
                    },
                    Self::ManifestDeclaration {
                        manifest: right_manifest,
                        span: right_span,
                        locator_kind: right_kind,
                    },
                ) => left_manifest
                    .cmp(right_manifest)
                    .then_with(|| left_span.start.cmp(&right_span.start))
                    .then_with(|| left_span.end.cmp(&right_span.end))
                    .then_with(|| left_kind.cmp(right_kind)),
                (
                    Self::ArtifactDependencyRecord {
                        artifact: left_artifact,
                        record_index: left_index,
                    },
                    Self::ArtifactDependencyRecord {
                        artifact: right_artifact,
                        record_index: right_index,
                    },
                ) => left_artifact
                    .cmp(right_artifact)
                    .then_with(|| left_index.cmp(right_index)),
                (
                    Self::InjectedTrustedCore {
                        dependent: left_dependent,
                    },
                    Self::InjectedTrustedCore {
                        dependent: right_dependent,
                    },
                ) => left_dependent.cmp(right_dependent),
                (Self::SyntheticSingleFileCore, Self::SyntheticSingleFileCore) => Ordering::Equal,
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for EdgeOrigin {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const fn edge_origin_rank(origin: &EdgeOrigin) -> u8 {
    match origin {
        EdgeOrigin::ManifestDeclaration { .. } => 0,
        EdgeOrigin::ArtifactDependencyRecord { .. } => 1,
        EdgeOrigin::InjectedTrustedCore { .. } => 2,
        EdgeOrigin::SyntheticSingleFileCore => 3,
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ManifestLocatorKind {
    SourcePath,
    ArtifactPath,
    SearchRoots,
}

#[derive(Debug)]
pub(crate) enum GraphNode {
    ManifestSource(Box<LoadedConeManifest>),
    Prebuilt(Box<PrebuiltArtifactProjection>),
    TrustedCore(Box<LoadedConeManifest>),
    SingleFile(SingleFileLocator),
}

impl GraphNode {
    pub(crate) fn coordinate(&self) -> &ConeCoordinate {
        match self {
            Self::ManifestSource(manifest) | Self::TrustedCore(manifest) => manifest.coordinate(),
            Self::Prebuilt(prebuilt) => prebuilt.coordinate(),
            Self::SingleFile(source) => {
                let _ = source.resolved_path();
                single_file_coordinate()
            }
        }
    }

    pub(crate) const fn representation(&self) -> DiscoveredNodeRepresentation {
        match self {
            Self::ManifestSource(_) => DiscoveredNodeRepresentation::ManifestSource,
            Self::Prebuilt(_) => DiscoveredNodeRepresentation::PrebuiltArtifact,
            Self::TrustedCore(_) => DiscoveredNodeRepresentation::TrustedCore,
            Self::SingleFile(_) => DiscoveredNodeRepresentation::SingleFile,
        }
    }
}

fn single_file_coordinate() -> &'static ConeCoordinate {
    static COORDINATE: std::sync::OnceLock<ConeCoordinate> = std::sync::OnceLock::new();
    COORDINATE.get_or_init(ConeCoordinate::reserved_single_file)
}

struct DiscoveryBuilder {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    explicit: Vec<PendingExplicitDependency>,
    unlocated: Vec<PendingUnlocatedDependency>,
    context: BuildContext,
    meter: SlibClosureDecodeMeterV1,
}

impl DiscoveryBuilder {
    fn new(loaded: LoadedBuildRoot) -> Result<Self, BuildGraphDiscoveryError> {
        let LoadedBuildRoot {
            root,
            trusted_core,
            context,
            meter,
        } = loaded;
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
            meter,
        };
        builder.insert_node(
            ConeIdentity::CORE,
            GraphNode::TrustedCore(Box::new(trusted_core)),
        )?;
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

    fn run(mut self) -> Result<DiscoveredBuildGraph, BuildGraphDiscoveryError> {
        while let Some(pending) = pop_explicit(&mut self.explicit) {
            let claim = {
                let (nodes, meter) = (&self.nodes, &mut self.meter);
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
                    meter,
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
                &mut self.meter,
            )
            .map_err(|error| BuildGraphDiscoveryError::Locator(Box::new(error)))?;
            self.intern_claim(LocatedDependencyClaim::Prebuilt(Box::new(claim)))?;
        }

        Ok(DiscoveredBuildGraph {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            context: self.context,
            meter: self.meter,
        })
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
            Some(GraphNode::TrustedCore(_) | GraphNode::SingleFile(_)) => {
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
                self.meter
                    .charge_graph(1, 0, 0)
                    .map_err(BuildGraphDiscoveryError::Resource)?;
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
            Some(GraphNode::TrustedCore(_) | GraphNode::SingleFile(_)) => {
                Err(BuildGraphDiscoveryError::ReservedNodeClaim(identity))
            }
        }
    }

    fn insert_node(
        &mut self,
        identity: ConeIdentity,
        node: GraphNode,
    ) -> Result<(), BuildGraphDiscoveryError> {
        self.meter
            .charge_graph(1, 0, 0)
            .map_err(BuildGraphDiscoveryError::Resource)?;
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
        self.meter
            .charge_graph(0, 1, 0)
            .map_err(BuildGraphDiscoveryError::Resource)?;
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
        self.insert_edge(DiscoveredDependencyEdge::new(
            identity,
            ConeIdentity::CORE,
            ConeCoordinate::reserved_core(),
            EdgeOrigin::InjectedTrustedCore {
                dependent: identity,
            },
            None,
        ))?;
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

struct ManifestDependencyPlan {
    coordinate: ConeCoordinate,
    locator: ManifestDependencyPlanLocator,
    edge: DiscoveredDependencyEdge,
}

enum ManifestDependencyPlanLocator {
    Explicit(DependencyCoordinateKey),
    SearchRoots,
}

fn manifest_dependency_plans(
    manifest: &LoadedConeManifest,
) -> Result<Vec<ManifestDependencyPlan>, BuildGraphDiscoveryError> {
    let dependent = manifest.identity();
    manifest
        .parsed()
        .semantic()
        .dependency_iter()
        .map(|(key, coordinate)| {
            let dependency = coordinate
                .identity()
                .map_err(BuildGraphDiscoveryError::Identity)?;
            let locator =
                manifest.parsed().locators().get(key).ok_or_else(|| {
                    BuildGraphDiscoveryError::MissingLocatorProjection(key.clone())
                })?;
            let span = manifest
                .parsed()
                .diagnostic_spans()
                .dependency(key)
                .ok_or_else(|| BuildGraphDiscoveryError::MissingDependencySpan(key.clone()))?
                .declaration()
                .range();
            let (locator_kind, plan_locator) = match locator {
                DependencyLocator::SourcePath(_) => (
                    ManifestLocatorKind::SourcePath,
                    ManifestDependencyPlanLocator::Explicit(key.clone()),
                ),
                DependencyLocator::ArtifactPath(_) => (
                    ManifestLocatorKind::ArtifactPath,
                    ManifestDependencyPlanLocator::Explicit(key.clone()),
                ),
                DependencyLocator::SearchRoots => (
                    ManifestLocatorKind::SearchRoots,
                    ManifestDependencyPlanLocator::SearchRoots,
                ),
            };
            Ok(ManifestDependencyPlan {
                coordinate: coordinate.clone(),
                locator: plan_locator,
                edge: DiscoveredDependencyEdge::new(
                    dependent,
                    dependency,
                    coordinate.clone(),
                    EdgeOrigin::ManifestDeclaration {
                        manifest: manifest.manifest_path().to_path_buf(),
                        span,
                        locator_kind,
                    },
                    None,
                ),
            })
        })
        .collect()
}

struct PrebuiltDependencyPlan {
    coordinate: ConeCoordinate,
    edge: DiscoveredDependencyEdge,
}

fn prebuilt_dependency_plans(
    dependent: ConeIdentity,
    prebuilt: &PrebuiltArtifactProjection,
) -> Vec<PrebuiltDependencyPlan> {
    let artifact = prebuilt.first_candidate().resolved_path().to_path_buf();
    prebuilt
        .first_candidate()
        .summary()
        .direct_dependencies()
        .iter()
        .enumerate()
        .map(|(record_index, dependency)| PrebuiltDependencyPlan {
            coordinate: dependency.coordinate().clone(),
            edge: DiscoveredDependencyEdge::new(
                dependent,
                dependency.identity(),
                dependency.coordinate().clone(),
                EdgeOrigin::ArtifactDependencyRecord {
                    artifact: artifact.clone(),
                    record_index,
                },
                Some(dependency.clone()),
            ),
        })
        .collect()
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

pub(crate) fn compare_coordinates(left: &ConeCoordinate, right: &ConeCoordinate) -> Ordering {
    left.group()
        .as_bytes()
        .cmp(right.group().as_bytes())
        .then_with(|| left.name().as_bytes().cmp(right.name().as_bytes()))
        .then_with(|| left.version().as_bytes().cmp(right.version().as_bytes()))
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

#[derive(Debug)]
pub enum LoadBuildRootError {
    RootManifest(ManifestRootError),
    TrustedCoreManifest(ManifestRootError),
    Resource(SlibClosureResourceErrorV1),
}

impl fmt::Display for LoadBuildRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootManifest(error) => write!(formatter, "invalid build root: {error}"),
            Self::TrustedCoreManifest(error) => write!(formatter, "invalid trusted core: {error}"),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LoadBuildRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootManifest(error) | Self::TrustedCoreManifest(error) => Some(error),
            Self::Resource(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum BuildGraphDiscoveryError {
    Locator(Box<DependencyLocatorError>),
    Resource(SlibClosureResourceErrorV1),
    Identity(HashError),
    MissingLocatorProjection(DependencyCoordinateKey),
    MissingDependencySpan(DependencyCoordinateKey),
    IdentityCoordinateConflict {
        identity: ConeIdentity,
        first: Box<ConeCoordinate>,
        second: Box<ConeCoordinate>,
    },
    ConflictingSourceLocator {
        coordinate: ConeCoordinate,
        first: PathBuf,
        second: PathBuf,
    },
    AmbiguousArtifact {
        coordinate: Box<ConeCoordinate>,
        first: scoop_slib::ArtifactFingerprint,
        second: scoop_slib::ArtifactFingerprint,
    },
    ConflictingNodeRepresentation {
        coordinate: ConeCoordinate,
    },
    ReservedNodeClaim(ConeIdentity),
    ConflictingDependencyEdge {
        dependent: ConeIdentity,
        dependency: ConeIdentity,
    },
    InternalSourceClaim(ConeIdentity),
    InternalPrebuiltClaim(ConeIdentity),
}

impl fmt::Display for BuildGraphDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locator(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
            Self::MissingLocatorProjection(key) => {
                write!(
                    formatter,
                    "manifest dependency {key} has no locator projection"
                )
            }
            Self::MissingDependencySpan(key) => {
                write!(
                    formatter,
                    "manifest dependency {key} has no diagnostic span"
                )
            }
            Self::IdentityCoordinateConflict {
                identity,
                first,
                second,
            } => write!(
                formatter,
                "Cone identity {identity} is claimed by both {first} and {second}"
            ),
            Self::ConflictingSourceLocator {
                coordinate,
                first,
                second,
            } => write!(
                formatter,
                "source Cone {coordinate} is claimed from both {} and {}",
                first.display(),
                second.display()
            ),
            Self::AmbiguousArtifact {
                coordinate,
                first,
                second,
            } => write!(
                formatter,
                "artifact Cone {coordinate} has conflicting fingerprints {first} and {second}"
            ),
            Self::ConflictingNodeRepresentation { coordinate } => write!(
                formatter,
                "Cone {coordinate} is claimed as both source and prebuilt artifact"
            ),
            Self::ReservedNodeClaim(identity) => {
                write!(
                    formatter,
                    "ordinary locator attempted to claim reserved Cone {identity}"
                )
            }
            Self::ConflictingDependencyEdge {
                dependent,
                dependency,
            } => write!(
                formatter,
                "Cone {dependent} has conflicting dependency records for {dependency}"
            ),
            Self::InternalSourceClaim(identity) => write!(
                formatter,
                "internal graph state lost source claim for Cone {identity}"
            ),
            Self::InternalPrebuiltClaim(identity) => write!(
                formatter,
                "internal graph state lost prebuilt claim for Cone {identity}"
            ),
        }
    }
}

impl std::error::Error for BuildGraphDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Locator(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Identity(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_hir::CanonicalHirFoundation;
    use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
    use scoop_manifest::ManifestRootLocator;
    use scoop_mir::CanonicalMirFoundation;
    use scoop_protocol::TargetSelectionRequestV1;
    use scoop_slib::{
        ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
        IdentityFoundationArtifactInput, ProducerRecord,
    };

    fn write_manifest(root: &std::path::Path, name: &str, dependencies: &str) {
        std::fs::create_dir_all(root).unwrap();
        std::fs::write(
            root.join("Cone.toml"),
            format!(
                "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"library\"\n{dependencies}"
            ),
        )
        .unwrap();
    }

    fn write_core(sysroot: &std::path::Path) {
        let root = sysroot.join("lib").join("scoop.core");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
        )
        .unwrap();
    }

    fn request(
        root: &std::path::Path,
        sysroot: &std::path::Path,
        search_roots: Vec<ArtifactSearchRoot>,
    ) -> BuildGraphRequest {
        BuildGraphRequest::new(
            crate::BuildRootInput::manifest(ManifestRootLocator::cone_directory(root)).unwrap(),
            search_roots,
            ArtifactCacheRoot::new(sysroot.join("cache")).unwrap(),
            TrustedSysrootRoot::new(sysroot).unwrap(),
            TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
            PairedScoopcLocator::new(sysroot.join("bin/scoopc")).unwrap(),
            DiagnosticsPolicy::Structured,
            BuildLimitsProfileV1::M23_DEFAULT,
        )
        .unwrap()
    }

    fn foundation_artifact(coordinate: ConeCoordinate, producer: &str) -> Vec<u8> {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let cone =
            ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
        IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
            ProducerRecord::new(producer).unwrap(),
            cone,
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            &hir,
            &mir,
            &lir,
        ))
        .unwrap()
        .as_bytes()
        .to_vec()
    }

    #[test]
    fn source_chain_is_discovered_before_any_source_tree_read() {
        let temp = tempfile::tempdir().unwrap();
        let sysroot = temp.path().join("sysroot");
        write_core(&sysroot);
        let root = temp.path().join("root");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        write_manifest(
            &root,
            "root",
            "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n",
        );
        write_manifest(
            &a,
            "a",
            "[dependencies]\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
        );
        write_manifest(&b, "b", "");

        let graph = request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover()
            .unwrap();
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 5);
        assert_eq!(
            graph.node_representation(
                ConeCoordinate::new("test", "b", "1.0.0")
                    .unwrap()
                    .identity()
                    .unwrap()
            ),
            Some(DiscoveredNodeRepresentation::ManifestSource)
        );
        assert_eq!(graph.decode_usage().source_files, 0);
    }

    #[test]
    fn diamond_source_claims_must_resolve_to_one_real_root() {
        let temp = tempfile::tempdir().unwrap();
        let sysroot = temp.path().join("sysroot");
        write_core(&sysroot);
        let root = temp.path().join("root");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        let shared_a = temp.path().join("shared-a");
        let shared_b = temp.path().join("shared-b");
        write_manifest(
            &root,
            "root",
            "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
        );
        write_manifest(
            &a,
            "a",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-a\" }\n",
        );
        write_manifest(
            &b,
            "b",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-b\" }\n",
        );
        write_manifest(&shared_a, "shared", "");
        write_manifest(&shared_b, "shared", "");

        assert!(matches!(
            request(&root, &sysroot, vec![])
                .load_root()
                .unwrap()
                .discover(),
            Err(BuildGraphDiscoveryError::ConflictingSourceLocator { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn source_symlink_aliases_to_one_real_root_merge() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let sysroot = temp.path().join("sysroot");
        write_core(&sysroot);
        let root = temp.path().join("root");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        let shared = temp.path().join("shared");
        write_manifest(
            &root,
            "root",
            "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
        );
        write_manifest(
            &a,
            "a",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-alias-a\" }\n",
        );
        write_manifest(
            &b,
            "b",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared-alias-b\" }\n",
        );
        write_manifest(&shared, "shared", "");
        symlink(&shared, temp.path().join("shared-alias-a")).unwrap();
        symlink(&shared, temp.path().join("shared-alias-b")).unwrap();

        let graph = request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover()
            .unwrap();
        let shared_identity = ConeCoordinate::new("test", "shared", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        assert_eq!(graph.node_count(), 5);
        assert_eq!(
            graph.node_representation(shared_identity),
            Some(DiscoveredNodeRepresentation::ManifestSource)
        );
    }

    #[test]
    fn identical_artifact_claims_merge_but_different_fingerprints_conflict() {
        let temp = tempfile::tempdir().unwrap();
        let sysroot = temp.path().join("sysroot");
        write_core(&sysroot);
        let root = temp.path().join("root");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        write_manifest(
            &root,
            "root",
            "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
        );
        write_manifest(
            &a,
            "a",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared-a.slib\" }\n",
        );
        write_manifest(
            &b,
            "b",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared-b.slib\" }\n",
        );
        let shared = ConeCoordinate::new("test", "shared", "1.0.0").unwrap();
        let bytes = foundation_artifact(shared.clone(), "same");
        std::fs::write(temp.path().join("shared-a.slib"), &bytes).unwrap();
        std::fs::write(temp.path().join("shared-b.slib"), &bytes).unwrap();

        let graph = request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover()
            .unwrap();
        assert_eq!(
            graph.prebuilt_candidate_count(shared.identity().unwrap()),
            Some(2)
        );

        std::fs::write(
            temp.path().join("shared-b.slib"),
            foundation_artifact(shared, "different"),
        )
        .unwrap();
        assert!(matches!(
            request(&root, &sysroot, vec![])
                .load_root()
                .unwrap()
                .discover(),
            Err(BuildGraphDiscoveryError::AmbiguousArtifact { .. })
        ));
    }

    #[test]
    fn source_and_artifact_claims_never_select_by_discovery_order() {
        let temp = tempfile::tempdir().unwrap();
        let sysroot = temp.path().join("sysroot");
        write_core(&sysroot);
        let root = temp.path().join("root");
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        let shared_source = temp.path().join("shared");
        write_manifest(
            &root,
            "root",
            "[dependencies]\n\"test:a\" = { version = \"1.0.0\", path = \"../a\" }\n\"test:b\" = { version = \"1.0.0\", path = \"../b\" }\n",
        );
        write_manifest(
            &a,
            "a",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", path = \"../shared\" }\n",
        );
        write_manifest(
            &b,
            "b",
            "[dependencies]\n\"test:shared\" = { version = \"1.0.0\", artifact = \"../shared.slib\" }\n",
        );
        write_manifest(&shared_source, "shared", "");
        let shared = ConeCoordinate::new("test", "shared", "1.0.0").unwrap();
        std::fs::write(
            temp.path().join("shared.slib"),
            foundation_artifact(shared, "artifact"),
        )
        .unwrap();

        assert!(matches!(
            request(&root, &sysroot, vec![])
                .load_root()
                .unwrap()
                .discover(),
            Err(BuildGraphDiscoveryError::ConflictingNodeRepresentation { .. })
        ));
    }
}
