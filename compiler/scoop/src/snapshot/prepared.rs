use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_manifest::{LoadedConeManifest, select_manifest_sources, select_single_file_source};
use scoop_slib::ArtifactSnapshot;

use super::staging::PreparedStaging;
use crate::discovery::{BuildContext, GraphNode};
use crate::graph::ResolvedGraphParts;
use crate::locator::{PrebuiltArtifactCandidate, PrebuiltArtifactProjection};
use crate::{ImmutableInputSnapshot, ResolvedBuildGraph, ResolvedPairedScoopc};

mod error;
mod model;
mod observation;
mod ordinary_execution;

pub use error::PrepareBuildGraphError;
pub use observation::{BuildDumpRequest, DumpScope};
pub use ordinary_execution::OrdinarySourceExecutionError;

pub use model::{
    ChildRequestPlanError, CompileCacheKeyError, ManifestSourceInput, PreparedArtifactCandidate,
    PreparedBuildGraph, PreparedNodeRepresentation, SelectedSource, SingleFileSourceInput,
};

use model::{
    NonEmptyPreparedArtifactCandidates, PreparedGraphNode, PreparedManifestSourceNode,
    PreparedPrebuiltArtifactNode, PreparedSingleFileNode,
};

impl ResolvedBuildGraph {
    /// Prepares the graph and actual input paths for compiler child execution.
    pub fn prepare(self) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
        Preparer::new(self.into_parts())?.run()
    }
}

struct Preparer {
    parts: ResolvedGraphParts,
    staging: PreparedStaging,
    compiler: ResolvedPairedScoopc,
}

impl Preparer {
    fn new(parts: ResolvedGraphParts) -> Result<Self, PrepareBuildGraphError> {
        let compiler =
            ResolvedPairedScoopc::resolve(&parts.context.compiler, &parts.context.cache_root)
                .map_err(PrepareBuildGraphError::PairedCompiler)?;
        let staging = PreparedStaging::create(&parts.context.cache_root)
            .map_err(PrepareBuildGraphError::Staging)?;
        Ok(Self {
            parts,
            staging,
            compiler,
        })
    }

    fn run(self) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
        let topological_indices = self
            .parts
            .dependency_first
            .iter()
            .copied()
            .enumerate()
            .map(|(index, identity)| (identity, index))
            .collect::<BTreeMap<_, _>>();
        let mut nodes = BTreeMap::new();

        for (identity, node) in self.parts.nodes {
            let prepared = match node {
                GraphNode::ManifestSource(manifest) => {
                    let purpose = if identity == self.parts.root {
                        self.parts.context.purpose
                    } else {
                        scoop_protocol::CompilationPurpose::Production
                    };
                    let input_root = manifest.real_root().to_path_buf();
                    let snapshot = select_manifest_input(*manifest, &self.parts.context, purpose)?;
                    let native_inputs = snapshot
                        .native
                        .units()
                        .iter()
                        .enumerate()
                        .map(|(index, unit)| {
                            self.staging
                                .write_native_input(
                                    &identity.to_string(),
                                    index,
                                    unit.preprocessed(),
                                )
                                .map_err(PrepareBuildGraphError::Staging)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let output_path = self
                        .staging
                        .plan_output(topological_indices[&identity], &identity.to_string())
                        .map_err(PrepareBuildGraphError::Staging)?;
                    PreparedGraphNode::ManifestSource(Box::new(PreparedManifestSourceNode {
                        snapshot,
                        input_root,
                        output_path,
                        native_inputs,
                    }))
                }
                GraphNode::Prebuilt(prebuilt) => {
                    PreparedGraphNode::PrebuiltArtifact(Box::new(PreparedPrebuiltArtifactNode {
                        coordinate: prebuilt.coordinate().clone(),
                        artifact_fingerprint: prebuilt.artifact_fingerprint(),
                        candidates: prepare_prebuilt_candidates(*prebuilt, &self.parts.context)?,
                    }))
                }
                GraphNode::SingleFile(locator) => {
                    let discovered = select_single_file_source(&locator)
                        .map_err(PrepareBuildGraphError::SingleFile)?;
                    let snapshot = SingleFileSourceInput { source: discovered };
                    let input_path = locator.resolved_path().to_path_buf();
                    let output_path = self
                        .staging
                        .plan_output(topological_indices[&identity], &identity.to_string())
                        .map_err(PrepareBuildGraphError::Staging)?;
                    PreparedGraphNode::SingleFile(Box::new(PreparedSingleFileNode {
                        snapshot,
                        input_path,
                        output_path,
                    }))
                }
            };
            nodes.insert(identity, prepared);
        }
        Ok(PreparedBuildGraph {
            root: self.parts.root,
            nodes,
            edges: self.parts.edges,
            dependency_first: self.parts.dependency_first,
            source_inputs: self.parts.source_inputs,
            root_kind: self.parts.root_kind,
            target_selection: self.parts.target_selection,
            context: self.parts.context,

            staging: self.staging,
            compiler: self.compiler,
            dumps: BTreeMap::new(),
        })
    }
}

fn select_manifest_input(
    manifest: LoadedConeManifest,
    context: &BuildContext,
    purpose: scoop_protocol::CompilationPurpose,
) -> Result<ManifestSourceInput, PrepareBuildGraphError> {
    let discovered = match purpose {
        scoop_protocol::CompilationPurpose::Production => {
            select_manifest_sources(&manifest, context.target.id())
        }
        scoop_protocol::CompilationPurpose::Test => {
            scoop_manifest::select_manifest_sources_for_tests(&manifest, context.target.id())
        }
    }
    .map_err(PrepareBuildGraphError::SourceDiscovery)?;

    let coordinate = manifest.coordinate().clone();
    let identity = manifest.identity();
    let requested_kind = match purpose {
        scoop_protocol::CompilationPurpose::Production => {
            manifest.parsed().semantic().requested_kind()
        }
        scoop_protocol::CompilationPurpose::Test => scoop_identity::RequestedConeKind::Executable,
    };
    let test_config = (purpose == scoop_protocol::CompilationPurpose::Test)
        .then(|| manifest.parsed().tests().clone());
    let manifest_semantic = manifest.parsed().semantic().clone();
    let native = scoop_toolchain::prepare_native_inputs(
        &manifest,
        context.target.id(),
        context.target.c_bridge_toolchain(),
        context.optimization,
        &scoop_toolchain::development_runtime_root().join("include"),
    )
    .map_err(PrepareBuildGraphError::Native)?;
    Ok(ManifestSourceInput {
        identity,
        coordinate,
        requested_kind,
        manifest_semantic,
        test_config,
        native,
        manifest_locator: manifest.manifest_path().to_path_buf(),
        sources: discovered,
    })
}

fn prepare_prebuilt_candidates(
    prebuilt: PrebuiltArtifactProjection,
    context: &BuildContext,
) -> Result<NonEmptyPreparedArtifactCandidates, PrepareBuildGraphError> {
    let (coordinate, artifact_fingerprint, first, rest) = prebuilt.into_parts();
    let first = prepare_artifact_candidate(first, context)?;
    let rest = rest
        .into_iter()
        .map(|candidate| prepare_artifact_candidate(candidate, context))
        .collect::<Result<Vec<_>, _>>()?;
    if first.summary.cone().coordinate() != &coordinate
        || first.summary.artifact_fingerprint() != artifact_fingerprint
        || rest.iter().any(|candidate| {
            candidate.summary.cone().coordinate() != &coordinate
                || candidate.summary.artifact_fingerprint() != artifact_fingerprint
        })
    {
        return Err(PrepareBuildGraphError::PrebuiltProjectionChanged(
            coordinate,
        ));
    }
    Ok(NonEmptyPreparedArtifactCandidates { first, rest })
}

fn prepare_artifact_candidate(
    candidate: PrebuiltArtifactCandidate,
    context: &BuildContext,
) -> Result<PreparedArtifactCandidate, PrepareBuildGraphError> {
    let (source_locator, expected_summary) = candidate.into_parts();
    let input = ImmutableInputSnapshot::capture(&source_locator).map_err(|source| {
        PrepareBuildGraphError::ArtifactSnapshot {
            path: source_locator.clone(),
            source,
        }
    })?;

    let snapshot = Arc::new(ArtifactSnapshot::from_shared(input.shared_bytes()));
    let summary = snapshot
        .manifest_summary(context.target.lir_target_selection())
        .map_err(|source| PrepareBuildGraphError::ArtifactSummary {
            path: source_locator.clone(),
            source,
        })?;
    if summary != expected_summary {
        return Err(PrepareBuildGraphError::ArtifactSummaryChanged(
            source_locator,
        ));
    }

    Ok(PreparedArtifactCandidate {
        source_locator,
        snapshot,
        summary,
    })
}

#[cfg(test)]
mod tests;
