use std::collections::BTreeMap;

use scoop_manifest::{LoadedConeManifest, select_manifest_sources, select_single_file_source};

use super::staging::PreparedStaging;
use crate::discovery::{BuildContext, GraphNode};
use crate::graph::ResolvedGraphParts;
use crate::locator::{PrebuiltArtifactCandidate, PrebuiltArtifactProjection};
use crate::{ResolvedBuildGraph, ResolvedPairedScoopc};

mod documented_execution;
mod error;
mod model;
mod observation;
mod ordinary_execution;

pub use error::PrepareBuildGraphError;
pub use observation::{BuildDumpRequest, DumpScope};
pub use ordinary_execution::OrdinarySourceExecutionError;
pub(crate) use ordinary_execution::{PendingSourceExecution, PreparedSourceExecution};

pub use model::{
    ChildRequestPlanError, CompileCacheKeyError, ManifestSourceInput, PreparedArtifactCandidate,
    PreparedBuildGraph, PreparedNodeRepresentation, SelectedSource, SingleFileSourceInput,
};

use model::{
    NonEmptyPreparedArtifactCandidates, PreparedGraphNode, PreparedManifestSourceNode,
    PreparedPrebuiltArtifactNode, PreparedSingleFileNode, SourceNativeInput,
};

impl ResolvedBuildGraph {
    /// Prepares the graph and actual input paths for compiler child execution.
    pub fn prepare(self) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
        Preparer::new(self.into_parts(), false)?.run()
    }

    pub(crate) fn prepare_documentation(
        self,
    ) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
        Preparer::new(self.into_parts(), true)?.run()
    }
}

impl PreparedBuildGraph {
    pub(crate) fn prepare_machine_source(
        &mut self,
        identity: scoop_identity::ConeIdentity,
    ) -> Result<(), PrepareBuildGraphError> {
        let Some(PreparedGraphNode::ManifestSource(node)) = self.nodes.get_mut(&identity) else {
            return Ok(());
        };
        if matches!(node.snapshot.native, SourceNativeInput::Artifact(_)) {
            return Ok(());
        }
        let target = self
            .context
            .target
            .production(self.context.cache_root.as_path())
            .map_err(PrepareBuildGraphError::Native)?
            .into_owned();
        let manifest = self
            .context
            .cache_root
            .load_manifest(&scoop_manifest::ManifestRootLocator::exact_manifest_file(
                &node.snapshot.manifest_locator,
            ))
            .map_err(PrepareBuildGraphError::Manifest)?;
        let native = scoop_toolchain::prepare_native_objects(
            &manifest,
            &target,
            self.context.optimization,
            &self.context.sysroot.as_path().join("include"),
            self.context.cache_root.as_path(),
        )
        .map_err(PrepareBuildGraphError::Native)?;
        node.snapshot.native = SourceNativeInput::Artifact(native);
        self.context.target = crate::GraphTarget::Artifact(Box::new(target));
        Ok(())
    }
}

struct Preparer {
    parts: ResolvedGraphParts,
    staging: PreparedStaging,
    compiler: ResolvedPairedScoopc,
    documentation: bool,
}

impl Preparer {
    fn new(parts: ResolvedGraphParts, documentation: bool) -> Result<Self, PrepareBuildGraphError> {
        let compiler =
            ResolvedPairedScoopc::resolve(&parts.context.compiler, &parts.context.cache_root)
                .map_err(PrepareBuildGraphError::PairedCompiler)?;
        let staging = PreparedStaging::create(&parts.context.cache_root)
            .map_err(PrepareBuildGraphError::Staging)?;
        Ok(Self {
            parts,
            staging,
            compiler,
            documentation,
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
                    let snapshot = select_manifest_input(
                        *manifest,
                        &self.parts.context,
                        purpose,
                        self.documentation,
                    )?;
                    let output_path = self
                        .staging
                        .plan_output(topological_indices[&identity], &identity.to_string())
                        .map_err(PrepareBuildGraphError::Staging)?;
                    PreparedGraphNode::ManifestSource(Box::new(PreparedManifestSourceNode {
                        snapshot,
                        input_root,
                        output_path,
                    }))
                }
                GraphNode::Prebuilt(prebuilt) => {
                    PreparedGraphNode::PrebuiltArtifact(Box::new(PreparedPrebuiltArtifactNode {
                        coordinate: prebuilt.coordinate().clone(),
                        artifact_fingerprint: prebuilt.artifact_fingerprint(),
                        candidates: prepare_prebuilt_candidates(*prebuilt),
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
    documentation: bool,
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
    let native = if documentation {
        SourceNativeInput::Documentation
    } else {
        let target = context
            .target
            .production(context.cache_root.as_path())
            .map_err(PrepareBuildGraphError::Native)?;
        SourceNativeInput::Artifact(
            scoop_toolchain::prepare_native_objects(
                &manifest,
                &target,
                context.optimization,
                &context.sysroot.as_path().join("include"),
                context.cache_root.as_path(),
            )
            .map_err(PrepareBuildGraphError::Native)?,
        )
    };
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
) -> NonEmptyPreparedArtifactCandidates {
    let (_, _, first, rest) = prebuilt.into_parts();
    NonEmptyPreparedArtifactCandidates {
        first: prepare_artifact_candidate(first),
        rest: rest.into_iter().map(prepare_artifact_candidate).collect(),
    }
}

fn prepare_artifact_candidate(candidate: PrebuiltArtifactCandidate) -> PreparedArtifactCandidate {
    PreparedArtifactCandidate {
        artifact: candidate.into_artifact(),
    }
}

#[cfg(test)]
mod tests;
