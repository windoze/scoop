use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use scoop_identity::{ConeIdentity, SourceContentDigest};
use scoop_manifest::{
    LoadedConeManifest, SourceDiscoveryLimits, discover_manifest_sources_with_limits,
    load_single_file_source_with_limit,
};
use scoop_slib::{
    ArtifactSnapshot, SlibClosureDecodeMeterV1, SlibClosureDecodePurposeV1,
    SlibClosureResourceErrorV1,
};
use scoop_wire::Digest256;

use super::staging::PreparedStaging;
use crate::discovery::{BuildContext, GraphNode};
use crate::graph::ResolvedGraphParts;
use crate::locator::{PrebuiltArtifactCandidate, PrebuiltArtifactProjection};
use crate::{ImmutableInputSnapshot, ResolvedBuildGraph, ResolvedPairedScoopc};

mod error;
mod model;
mod ordinary_execution;

pub use error::PrepareBuildGraphError;
pub use ordinary_execution::OrdinarySourceExecutionError;

pub use model::{
    ChildRequestPlanError, CompileCacheKeyError, ManifestSourceSnapshot, PreparedArtifactCandidate,
    PreparedBuildGraph, PreparedNodeRepresentation, SingleFileSourceSnapshot, SourceSnapshot,
};

use model::{
    NonEmptyPreparedArtifactCandidates, NonEmptySourceSnapshots, PreparedGraphNode,
    PreparedManifestSourceNode, PreparedPrebuiltArtifactNode, PreparedSingleFileNode,
};

impl ResolvedBuildGraph {
    /// Atomically snapshots every graph input and validates the paired compiler
    /// before granting the only state that may launch compiler children.
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
        let compiler = ResolvedPairedScoopc::resolve(&parts.context.compiler)
            .map_err(PrepareBuildGraphError::PairedCompiler)?;
        let staging = PreparedStaging::create(&parts.context.cache_root)
            .map_err(PrepareBuildGraphError::Staging)?;
        Ok(Self {
            parts,
            staging,
            compiler,
        })
    }

    fn run(mut self) -> Result<PreparedBuildGraph, PrepareBuildGraphError> {
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
                    let snapshot = capture_manifest_source(
                        *manifest,
                        &mut self.parts.meter,
                        self.parts
                            .context
                            .limits
                            .artifact_decode()
                            .semantic_leaf_bytes,
                    )?;
                    let input_root =
                        materialize_manifest_snapshot(&self.staging, identity, &snapshot)?;
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
                        candidates: prepare_prebuilt_candidates(
                            identity,
                            *prebuilt,
                            &mut self.parts.meter,
                            &self.parts.context,
                            &self.staging,
                        )?,
                    }))
                }
                GraphNode::SingleFile(locator) => {
                    let remaining = remaining_source_budget(&self.parts.meter)?;
                    if remaining.files() == 0 {
                        let observed = self.parts.meter.usage().source_files.checked_add(1).ok_or(
                            PrepareBuildGraphError::Resource(
                                SlibClosureResourceErrorV1::Overflow {
                                    resource: scoop_slib::SlibClosureResourceKindV1::SourceFiles,
                                },
                            ),
                        )?;
                        return Err(PrepareBuildGraphError::Resource(
                            SlibClosureResourceErrorV1::LimitExceeded {
                                resource: scoop_slib::SlibClosureResourceKindV1::SourceFiles,
                                limit: self.parts.meter.limits().values().source_files,
                                observed,
                            },
                        ));
                    }
                    let discovered =
                        load_single_file_source_with_limit(&locator, remaining.bytes())
                            .map_err(PrepareBuildGraphError::SingleFile)?;
                    let byte_length = u64::try_from(discovered.source_text().len())
                        .map_err(|_| PrepareBuildGraphError::SourceLengthOverflow)?;
                    self.parts
                        .meter
                        .charge_sources(1, byte_length)
                        .map_err(PrepareBuildGraphError::Resource)?;
                    let snapshot = SingleFileSourceSnapshot {
                        source: SourceSnapshot::from_discovered(discovered),
                    };
                    let input_path = self
                        .staging
                        .materialize_single_file(
                            &identity.to_string(),
                            snapshot.source.as_bytes(),
                            digest_from_source(snapshot.source.content_digest()),
                        )
                        .map_err(PrepareBuildGraphError::Staging)?;
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
        self.staging
            .seal_inputs()
            .map_err(PrepareBuildGraphError::Staging)?;
        Ok(PreparedBuildGraph {
            root: self.parts.root,
            nodes,
            edges: self.parts.edges,
            dependency_first: self.parts.dependency_first,
            source_inputs: self.parts.source_inputs,
            root_kind: self.parts.root_kind,
            target_selection: self.parts.target_selection,
            context: self.parts.context,
            meter: self.parts.meter,
            staging: self.staging,
            compiler: self.compiler,
        })
    }
}

fn capture_manifest_source(
    manifest: LoadedConeManifest,
    meter: &mut SlibClosureDecodeMeterV1,
    manifest_byte_limit: u64,
) -> Result<ManifestSourceSnapshot, PrepareBuildGraphError> {
    let manifest_input =
        ImmutableInputSnapshot::capture(manifest.manifest_path(), manifest_byte_limit)
            .map_err(PrepareBuildGraphError::ManifestSnapshot)?;
    if manifest_input.as_bytes() != manifest.source_bytes() {
        return Err(PrepareBuildGraphError::ManifestChanged(
            manifest.manifest_path().to_path_buf(),
        ));
    }
    let remaining = remaining_source_budget(meter)?;
    let discovered = discover_manifest_sources_with_limits(&manifest, remaining)
        .map_err(PrepareBuildGraphError::SourceDiscovery)?;
    let (first, rest, usage) = discovered.into_parts();
    meter
        .charge_sources(usage.files(), usage.bytes())
        .map_err(PrepareBuildGraphError::Resource)?;
    let coordinate = manifest.coordinate().clone();
    let identity = manifest.identity();
    let requested_kind = manifest.parsed().semantic().requested_kind();
    let manifest_semantic = manifest.parsed().semantic().clone();
    Ok(ManifestSourceSnapshot {
        identity,
        coordinate,
        requested_kind,
        manifest_semantic,
        manifest_locator: manifest.manifest_path().to_path_buf(),
        manifest_bytes: manifest_input.shared_bytes(),
        manifest_digest: manifest_input.digest(),
        sources: NonEmptySourceSnapshots {
            first: SourceSnapshot::from_discovered(first),
            rest: rest
                .into_iter()
                .map(SourceSnapshot::from_discovered)
                .collect(),
        },
    })
}

fn remaining_source_budget(
    meter: &SlibClosureDecodeMeterV1,
) -> Result<SourceDiscoveryLimits, PrepareBuildGraphError> {
    let limits = meter.limits().values();
    let usage = meter.usage();
    let files = limits.source_files.checked_sub(usage.source_files).ok_or(
        PrepareBuildGraphError::Resource(SlibClosureResourceErrorV1::Overflow {
            resource: scoop_slib::SlibClosureResourceKindV1::SourceFiles,
        }),
    )?;
    let bytes = limits.source_bytes.checked_sub(usage.source_bytes).ok_or(
        PrepareBuildGraphError::Resource(SlibClosureResourceErrorV1::Overflow {
            resource: scoop_slib::SlibClosureResourceKindV1::SourceBytes,
        }),
    )?;
    Ok(SourceDiscoveryLimits::new(files, bytes))
}

fn materialize_manifest_snapshot(
    staging: &PreparedStaging,
    identity: ConeIdentity,
    snapshot: &ManifestSourceSnapshot,
) -> Result<PathBuf, PrepareBuildGraphError> {
    let cone_directory = identity.to_string();
    let root = staging
        .materialize_manifest(
            &cone_directory,
            snapshot.manifest_bytes(),
            snapshot.manifest_digest(),
        )
        .map_err(PrepareBuildGraphError::Staging)?;
    for source in snapshot.sources() {
        staging
            .materialize_source(
                &cone_directory,
                source.identity().logical_path().as_str(),
                source.as_bytes(),
                digest_from_source(source.content_digest()),
            )
            .map_err(PrepareBuildGraphError::Staging)?;
    }
    Ok(root)
}

fn prepare_prebuilt_candidates(
    identity: ConeIdentity,
    prebuilt: PrebuiltArtifactProjection,
    meter: &mut SlibClosureDecodeMeterV1,
    context: &BuildContext,
    staging: &PreparedStaging,
) -> Result<NonEmptyPreparedArtifactCandidates, PrepareBuildGraphError> {
    let (coordinate, artifact_fingerprint, first, rest) = prebuilt.into_parts();
    let first = prepare_artifact_candidate(identity, 0, first, meter, context, staging)?;
    let rest = rest
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            prepare_artifact_candidate(identity, index + 1, candidate, meter, context, staging)
        })
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
    identity: ConeIdentity,
    index: usize,
    candidate: PrebuiltArtifactCandidate,
    meter: &mut SlibClosureDecodeMeterV1,
    context: &BuildContext,
    staging: &PreparedStaging,
) -> Result<PreparedArtifactCandidate, PrepareBuildGraphError> {
    let (source_locator, expected_summary) = candidate.into_parts();
    let input = ImmutableInputSnapshot::capture(
        &source_locator,
        context.limits.artifact_decode().owned_bytes,
    )
    .map_err(|source| PrepareBuildGraphError::ArtifactSnapshot {
        path: source_locator.clone(),
        source,
    })?;
    let byte_length = u64::try_from(input.byte_length())
        .map_err(|_| PrepareBuildGraphError::ArtifactLengthOverflow(source_locator.clone()))?;
    meter
        .observe_raw_artifact_snapshot(input.digest(), byte_length)
        .map_err(PrepareBuildGraphError::Resource)?;
    let snapshot = Arc::new(ArtifactSnapshot::from_shared(input.shared_bytes()));
    let summary = snapshot
        .probe_prebuilt_summary(
            context.limits.artifact_decode(),
            context.target.lir_target_selection(),
        )
        .map_err(|source| PrepareBuildGraphError::ArtifactSummary {
            path: source_locator.clone(),
            source,
        })?;
    if summary != expected_summary {
        return Err(PrepareBuildGraphError::ArtifactSummaryChanged(
            source_locator,
        ));
    }
    meter
        .observe_artifact_snapshot(&summary, snapshot.digest())
        .map_err(PrepareBuildGraphError::Resource)?;
    meter
        .charge_artifact_decode(
            SlibClosureDecodePurposeV1::GraphSummary,
            summary.artifact_fingerprint(),
            snapshot.digest(),
            summary.decode_usage(),
        )
        .map_err(PrepareBuildGraphError::Resource)?;
    let materialized_path = staging
        .materialize_artifact(
            &identity.to_string(),
            index,
            snapshot.as_bytes(),
            snapshot.digest(),
        )
        .map_err(PrepareBuildGraphError::Staging)?;
    Ok(PreparedArtifactCandidate {
        source_locator,
        materialized_path,
        snapshot,
        summary,
    })
}

fn digest_from_source(digest: SourceContentDigest) -> Digest256 {
    Digest256::from_array(*digest.as_array())
}

#[cfg(test)]
mod tests;
