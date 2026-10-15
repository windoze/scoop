use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity, RequestedConeKind};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_manifest::{ConeManifestSemantic, SelectedManifestSources};
use scoop_slib::{ArtifactFingerprint, ArtifactManifestSummaryV1, ConeKind, ConeSourceForm};

use super::super::staging::PreparedStaging;
use crate::ResolvedPairedScoopc;
use crate::artifact::{ArtifactClosurePlan, PlannedArtifactEdge, PlannedArtifactNode};
use crate::artifact::{CompletedNode, PrebuiltCompletionError, complete_prebuilt_candidates};
use crate::discovery::BuildContext;
use crate::graph::{ResolvedDependencyEdge, ResolvedDependencyProjection};

mod cache_key;
pub(super) mod child_request;

pub use cache_key::CompileCacheKeyError;
pub use child_request::ChildRequestPlanError;

pub use scoop_manifest::SelectedSource;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSourceInput {
    pub(super) identity: ConeIdentity,
    pub(super) coordinate: ConeCoordinate,
    pub(super) requested_kind: RequestedConeKind,
    pub(super) manifest_semantic: ConeManifestSemantic,
    pub(super) test_config: Option<scoop_manifest::TestManifestConfig>,
    pub(super) manifest_locator: PathBuf,
    pub(super) sources: SelectedManifestSources,
    pub(super) native: scoop_toolchain::PreparedNativeObjects,
}

impl ManifestSourceInput {
    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn requested_kind(&self) -> RequestedConeKind {
        self.requested_kind
    }

    pub const fn manifest_semantic(&self) -> &ConeManifestSemantic {
        &self.manifest_semantic
    }

    pub fn manifest_locator(&self) -> &Path {
        &self.manifest_locator
    }

    pub fn sources(&self) -> impl Iterator<Item = &SelectedSource> {
        self.sources.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleFileSourceInput {
    pub(super) source: SelectedSource,
}

impl SingleFileSourceInput {
    pub const fn source(&self) -> &SelectedSource {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedArtifactCandidate {
    pub(super) artifact: std::rc::Rc<crate::BuildArtifact>,
}

impl PreparedArtifactCandidate {
    pub fn source_locator(&self) -> &Path {
        self.artifact.path()
    }

    pub fn summary(&self) -> &ArtifactManifestSummaryV1 {
        self.artifact.summary()
    }

    pub(crate) fn shared_artifact(&self) -> std::rc::Rc<crate::BuildArtifact> {
        std::rc::Rc::clone(&self.artifact)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NonEmptyPreparedArtifactCandidates {
    pub(super) first: PreparedArtifactCandidate,
    pub(super) rest: Vec<PreparedArtifactCandidate>,
}

impl NonEmptyPreparedArtifactCandidates {
    fn iter(&self) -> impl Iterator<Item = &PreparedArtifactCandidate> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedNodeRepresentation {
    ManifestSource,
    PrebuiltArtifact,
    SingleFile,
}

#[derive(Debug)]
pub(super) enum PreparedGraphNode {
    ManifestSource(Box<PreparedManifestSourceNode>),
    PrebuiltArtifact(Box<PreparedPrebuiltArtifactNode>),
    SingleFile(Box<PreparedSingleFileNode>),
}

#[derive(Debug)]
pub(super) struct PreparedManifestSourceNode {
    pub(super) snapshot: ManifestSourceInput,
    pub(super) input_root: PathBuf,
    pub(super) output_path: PathBuf,
}

#[derive(Debug)]
pub(super) struct PreparedPrebuiltArtifactNode {
    pub(super) coordinate: ConeCoordinate,
    pub(super) artifact_fingerprint: ArtifactFingerprint,
    pub(super) candidates: NonEmptyPreparedArtifactCandidates,
}

#[derive(Debug)]
pub(super) struct PreparedSingleFileNode {
    pub(super) snapshot: SingleFileSourceInput,
    pub(super) input_path: PathBuf,
    pub(super) output_path: PathBuf,
}

impl PreparedGraphNode {
    const fn representation(&self) -> PreparedNodeRepresentation {
        match self {
            Self::ManifestSource(_) => PreparedNodeRepresentation::ManifestSource,
            Self::PrebuiltArtifact(_) => PreparedNodeRepresentation::PrebuiltArtifact,
            Self::SingleFile(_) => PreparedNodeRepresentation::SingleFile,
        }
    }
}

#[derive(Debug)]
pub struct PreparedBuildGraph {
    pub(super) root: ConeIdentity,
    pub(super) nodes: BTreeMap<ConeIdentity, PreparedGraphNode>,
    pub(super) edges: BTreeMap<(ConeIdentity, ConeIdentity), ResolvedDependencyEdge>,
    pub(super) dependency_first: Vec<ConeIdentity>,
    pub(super) source_inputs: BTreeMap<ConeIdentity, ResolvedDependencyProjection>,
    pub(super) root_kind: RequestedConeKind,
    pub(super) target_selection: ValidatedLirTargetSelection,
    pub(super) context: BuildContext,

    pub(super) staging: PreparedStaging,
    pub(super) compiler: ResolvedPairedScoopc,
    pub(super) dumps: BTreeMap<ConeIdentity, super::observation::PreparedDump>,
}

impl PreparedBuildGraph {
    pub fn compilation_purpose(
        &self,
        identity: ConeIdentity,
    ) -> scoop_protocol::CompilationPurpose {
        if identity == self.root {
            self.context.purpose
        } else {
            scoop_protocol::CompilationPurpose::Production
        }
    }

    pub const fn root_identity(&self) -> ConeIdentity {
        self.root
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

    pub fn dependency_first(&self) -> &[ConeIdentity] {
        &self.dependency_first
    }

    pub fn node_representation(
        &self,
        identity: ConeIdentity,
    ) -> Option<PreparedNodeRepresentation> {
        self.nodes
            .get(&identity)
            .map(PreparedGraphNode::representation)
    }

    pub fn source_dependencies(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ResolvedDependencyProjection> {
        self.source_inputs.get(&identity)
    }

    pub fn source_input_path(&self, identity: ConeIdentity) -> Option<&Path> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::ManifestSource(node)) => Some(&node.input_root),
            Some(PreparedGraphNode::SingleFile(node)) => Some(&node.input_path),
            _ => None,
        }
    }

    pub fn source_input(&self, identity: ConeIdentity) -> Option<&ManifestSourceInput> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::ManifestSource(node)) => Some(&node.snapshot),
            _ => None,
        }
    }

    pub fn planned_output_path(&self, identity: ConeIdentity) -> Option<&Path> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::ManifestSource(node)) => Some(&node.output_path),
            Some(PreparedGraphNode::SingleFile(node)) => Some(&node.output_path),
            Some(PreparedGraphNode::PrebuiltArtifact(_)) | None => None,
        }
    }

    pub fn single_file_input(&self, identity: ConeIdentity) -> Option<&SingleFileSourceInput> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::SingleFile(node)) => Some(&node.snapshot),
            _ => None,
        }
    }

    pub fn prebuilt_candidates(
        &self,
        identity: ConeIdentity,
    ) -> Option<impl Iterator<Item = &PreparedArtifactCandidate>> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::PrebuiltArtifact(node)) => Some(node.candidates.iter()),
            _ => None,
        }
    }

    pub fn prebuilt_coordinate(&self, identity: ConeIdentity) -> Option<&ConeCoordinate> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::PrebuiltArtifact(node)) => Some(&node.coordinate),
            _ => None,
        }
    }

    pub fn prebuilt_artifact_fingerprint(
        &self,
        identity: ConeIdentity,
    ) -> Option<ArtifactFingerprint> {
        match self.nodes.get(&identity) {
            Some(PreparedGraphNode::PrebuiltArtifact(node)) => Some(node.artifact_fingerprint),
            _ => None,
        }
    }

    pub const fn compiler(&self) -> &ResolvedPairedScoopc {
        &self.compiler
    }

    pub fn staging_root(&self) -> &Path {
        self.staging.root()
    }

    pub fn output_root(&self) -> &Path {
        self.staging.output_root()
    }

    pub const fn diagnostics(&self) -> crate::DiagnosticsPolicy {
        self.context.diagnostics
    }

    /// Captures the exact graph shape used to match completed artifacts.
    pub fn artifact_closure_plan(&self) -> ArtifactClosurePlan {
        let nodes = self
            .nodes
            .iter()
            .map(|(identity, node)| {
                let planned = match node {
                    PreparedGraphNode::ManifestSource(node) => PlannedArtifactNode::new(
                        node.snapshot.coordinate.clone(),
                        cone_kind(node.snapshot.requested_kind),
                        ConeSourceForm::Manifest,
                        None,
                    ),
                    PreparedGraphNode::PrebuiltArtifact(node) => PlannedArtifactNode::new(
                        node.coordinate.clone(),
                        ConeKind::Library,
                        ConeSourceForm::Manifest,
                        Some(node.artifact_fingerprint),
                    ),
                    PreparedGraphNode::SingleFile(_) => PlannedArtifactNode::new(
                        ConeCoordinate::reserved_single_file(),
                        ConeKind::Executable,
                        ConeSourceForm::SingleFile,
                        None,
                    ),
                };
                (*identity, planned)
            })
            .collect();
        let edges = self.edges.values().map(|edge| {
            PlannedArtifactEdge::new(
                edge.dependent(),
                edge.dependency(),
                edge.expected_semantic().cloned(),
            )
        });
        ArtifactClosurePlan::new(
            self.root,
            self.target_selection,
            self.dependency_first.clone(),
            nodes,
            edges,
        )
    }

    /// Matches prebuilt manifest data against the completed dependencies.
    pub(crate) fn complete_prebuilt_node(
        &mut self,
        identity: ConeIdentity,
        completed: &[&CompletedNode],
    ) -> Result<CompletedNode, PrebuiltCompletionError> {
        let candidates: Vec<_> = match self.nodes.get(&identity) {
            Some(PreparedGraphNode::PrebuiltArtifact(node)) => {
                node.candidates.iter().cloned().collect()
            }
            _ => return Err(PrebuiltCompletionError::NotPrebuilt(identity)),
        };
        let plan = self.artifact_closure_plan();

        complete_prebuilt_candidates(&plan, identity, candidates, completed)
    }
}

const fn cone_kind(kind: RequestedConeKind) -> ConeKind {
    match kind {
        RequestedConeKind::Library => ConeKind::Library,
        RequestedConeKind::Executable => ConeKind::Executable,
    }
}
