use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::PathBuf;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::{
    DependencyCoordinateKey, DependencyLocator, LoadedConeManifest, ManifestRootError,
    SingleFileLocator, load_cone_manifest,
};
use scoop_slib::DependencyRecord;
use scoop_wire::HashError;

use crate::locator::{
    DependencyLocatorError, LocatedDependencyClaim, ManifestSourceProjection,
    PrebuiltArtifactProjection, locate_from_search_roots, locate_manifest_dependency,
};
use crate::request::{BuildGraphRequestParts, BuildRootInputKind};
use crate::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildGraphRequest, DiagnosticsPolicy,
    PairedScoopcLocator, TrustedSysrootRoot,
};

mod builder;
mod plans;
mod root;
use builder::DiscoveryBuilder;
pub use root::LoadedBuildRoot;
use root::LoadedRootInput;

#[derive(Debug)]
pub(crate) struct BuildContext {
    pub(crate) artifact_search_roots: Vec<ArtifactSearchRoot>,
    pub(crate) cache_root: ArtifactCacheRoot,
    pub(crate) sysroot: TrustedSysrootRoot,
    pub(crate) target: scoop_toolchain::ResolvedTargetProfile,
    pub(crate) compiler: PairedScoopcLocator,
    pub(crate) diagnostics: DiagnosticsPolicy,
}

#[derive(Debug)]
pub struct DiscoveredBuildGraph {
    root: ConeIdentity,
    nodes: BTreeMap<ConeIdentity, GraphNode>,
    edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    context: BuildContext,
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
            Some(GraphNode::ManifestSource(_) | GraphNode::SingleFile(_)) | None => None,
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

    pub(crate) fn into_parts(self) -> DiscoveredGraphParts {
        DiscoveredGraphParts {
            root: self.root,
            nodes: self.nodes,
            edges: self.edges,
            context: self.context,
        }
    }
}

pub(crate) struct DiscoveredGraphParts {
    pub(crate) root: ConeIdentity,
    pub(crate) nodes: BTreeMap<ConeIdentity, GraphNode>,
    pub(crate) edges: BTreeMap<(ConeIdentity, ConeIdentity), DiscoveredDependencyEdge>,
    pub(crate) context: BuildContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveredNodeRepresentation {
    ManifestSource,
    PrebuiltArtifact,
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
    SingleFile(SingleFileLocator),
}

impl GraphNode {
    pub(crate) fn coordinate(&self) -> &ConeCoordinate {
        match self {
            Self::ManifestSource(manifest) => manifest.coordinate(),
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
            Self::SingleFile(_) => DiscoveredNodeRepresentation::SingleFile,
        }
    }
}

fn single_file_coordinate() -> &'static ConeCoordinate {
    static COORDINATE: std::sync::OnceLock<ConeCoordinate> = std::sync::OnceLock::new();
    COORDINATE.get_or_init(ConeCoordinate::reserved_single_file)
}

pub(crate) fn compare_coordinates(left: &ConeCoordinate, right: &ConeCoordinate) -> Ordering {
    left.group()
        .as_bytes()
        .cmp(right.group().as_bytes())
        .then_with(|| left.name().as_bytes().cmp(right.name().as_bytes()))
        .then_with(|| left.version().as_bytes().cmp(right.version().as_bytes()))
}

#[derive(Debug)]
pub enum LoadBuildRootError {
    RootManifest(ManifestRootError),
}

impl fmt::Display for LoadBuildRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootManifest(error) => write!(formatter, "invalid build root: {error}"),
        }
    }
}

impl std::error::Error for LoadBuildRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootManifest(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum BuildGraphDiscoveryError {
    Locator(Box<DependencyLocatorError>),

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

            Self::Identity(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
