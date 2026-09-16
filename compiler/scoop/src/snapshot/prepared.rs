use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs4::FileExt;
use scoop_identity::{
    ArtifactCapabilityProfileId, ConeCoordinate, ConeIdentity, SourceContentDigest,
};
use scoop_manifest::{
    LoadedConeManifest, SourceDiscoveryLimits, discover_manifest_sources_with_limits,
    load_single_file_source_with_limit,
};
use scoop_slib::{
    ArtifactSnapshot, ConeKind, ConeSourceForm, SlibClosureDecodeMeterV1,
    SlibClosureDecodePurposeV1, SlibClosureResourceErrorV1,
};
use scoop_wire::Digest256;

use super::staging::PreparedStaging;
use crate::discovery::{BuildContext, GraphNode};
use crate::graph::ResolvedGraphParts;
use crate::locator::{PrebuiltArtifactCandidate, PrebuiltArtifactProjection};
use crate::{ImmutableInputSnapshot, ResolvedBuildGraph, ResolvedPairedScoopc};

mod core_execution;
mod error;
mod model;

pub use core_execution::CoreBootstrapExecutionError;
pub use error::{CoreLockOperation, PrepareBuildGraphError};

pub use model::{
    ChildInvocationPlanV1, ChildRequestPlanError, CompileCacheKeyError, CoreBootstrapReason,
    ManifestSourceSnapshot, PreparedArtifactCandidate, PreparedBuildGraph,
    PreparedNodeRepresentation, SingleFileSourceSnapshot, SourceSnapshot, TrustedCorePreparation,
};

use model::{
    NonEmptyPreparedArtifactCandidates, NonEmptySourceSnapshots, PreparedGraphNode,
    PreparedManifestSourceNode, PreparedPrebuiltArtifactNode, PreparedSingleFileNode,
    PreparedTrustedCoreNode, trusted_core_source_key,
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
        let core_layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
            self.parts.context.sysroot.as_path(),
            self.parts.target_selection,
        );
        let core_lock = CoreSlotLock::acquire(&core_layout)?;
        let core_node = self
            .parts
            .nodes
            .remove(&ConeIdentity::CORE)
            .ok_or(PrepareBuildGraphError::MissingTrustedCore)?;
        let GraphNode::TrustedCore(core_manifest) = core_node else {
            return Err(PrepareBuildGraphError::InvalidTrustedCoreRepresentation);
        };
        let core_snapshot = capture_manifest_source(
            *core_manifest,
            &mut self.parts.meter,
            self.parts
                .context
                .limits
                .artifact_decode()
                .semantic_leaf_bytes,
        )?;
        let core_source_key = trusted_core_source_key(
            &core_snapshot,
            &self.compiler,
            &self.parts.context,
            self.parts.target_selection,
        )
        .map_err(|source| PrepareBuildGraphError::CoreSourceKey(Box::new(source)))?;
        let (preparation, existing, receipt) = prepare_core_artifact(
            core_layout.artifact(),
            core_layout.receipt(),
            core_source_key,
            self.compiler.fingerprint(),
            &mut self.parts.meter,
            &self.parts.context,
            &mut self.staging,
        )?;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            ConeIdentity::CORE,
            PreparedGraphNode::TrustedCore(Box::new(PreparedTrustedCoreNode {
                snapshot: core_snapshot,
                source_key: core_source_key,
                preparation,
                existing,
                receipt,
                artifact_slot: core_layout.artifact().to_path_buf(),
                receipt_slot: core_layout.receipt().to_path_buf(),
            })),
        );

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
                    let output_path = self.staging.planned_output(&identity.to_string());
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
                    let output_path = self.staging.planned_output(&identity.to_string());
                    PreparedGraphNode::SingleFile(Box::new(PreparedSingleFileNode {
                        snapshot,
                        input_path,
                        output_path,
                    }))
                }
                GraphNode::TrustedCore(_) => {
                    return Err(PrepareBuildGraphError::DuplicateTrustedCore);
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
            core_lock,
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

fn prepare_core_artifact(
    artifact_slot: &Path,
    receipt_slot: &Path,
    expected_source_key: crate::CoreSourceSnapshotKeyV1,
    expected_compiler: crate::PairedCompilerFingerprintV1,
    meter: &mut SlibClosureDecodeMeterV1,
    context: &BuildContext,
    staging: &mut PreparedStaging,
) -> Result<
    (
        TrustedCorePreparation,
        Option<PreparedArtifactCandidate>,
        Option<crate::TrustedCoreSlotReceiptV1>,
    ),
    PrepareBuildGraphError,
> {
    let metadata = match std::fs::symlink_metadata(artifact_slot) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Missing),
                None,
                None,
            ));
        }
        Err(_) => {
            return Ok((
                TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Unreadable),
                None,
                None,
            ));
        }
    };
    if !metadata.file_type().is_file() {
        return Ok((
            TrustedCorePreparation::Bootstrap(CoreBootstrapReason::WrongFileType),
            None,
            None,
        ));
    }
    let input = match ImmutableInputSnapshot::capture(
        artifact_slot,
        context.limits.artifact_decode().owned_bytes,
    ) {
        Ok(input) => input,
        Err(_) => {
            return Ok((
                TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Corrupt),
                None,
                None,
            ));
        }
    };
    let byte_length = u64::try_from(input.byte_length())
        .map_err(|_| PrepareBuildGraphError::ArtifactLengthOverflow(artifact_slot.to_path_buf()))?;
    meter
        .observe_raw_artifact_snapshot(input.digest(), byte_length)
        .map_err(PrepareBuildGraphError::Resource)?;
    let snapshot = Arc::new(ArtifactSnapshot::from_shared(input.shared_bytes()));
    let summary = match snapshot.probe_prebuilt_summary(
        context.limits.artifact_decode(),
        context.target.lir_target_selection(),
    ) {
        Ok(summary) => summary,
        Err(_) => {
            return Ok((
                TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Corrupt),
                None,
                None,
            ));
        }
    };
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
    if summary.cone().coordinate() != &ConeCoordinate::reserved_core()
        || summary.cone().kind() != ConeKind::Library
        || summary.cone().source_form() != ConeSourceForm::Manifest
        || !summary.direct_dependencies().is_empty()
    {
        return Ok((
            TrustedCorePreparation::Bootstrap(CoreBootstrapReason::Incompatible),
            None,
            None,
        ));
    }
    let materialized_path = staging
        .materialize_artifact(
            &ConeIdentity::CORE.to_string(),
            0,
            snapshot.as_bytes(),
            snapshot.digest(),
        )
        .map_err(PrepareBuildGraphError::Staging)?;
    let candidate = PreparedArtifactCandidate {
        source_locator: artifact_slot.to_path_buf(),
        materialized_path,
        snapshot,
        summary,
    };
    let receipt = read_matching_core_receipt(
        receipt_slot,
        expected_source_key,
        expected_compiler,
        &candidate,
        context,
    );
    Ok(match receipt {
        Some(receipt) => (
            TrustedCorePreparation::ReuseVerifiedSlot,
            Some(candidate),
            Some(receipt),
        ),
        None => (
            TrustedCorePreparation::Bootstrap(CoreBootstrapReason::ReceiptUnavailable),
            Some(candidate),
            None,
        ),
    })
}

fn read_matching_core_receipt(
    receipt_slot: &Path,
    expected_source_key: crate::CoreSourceSnapshotKeyV1,
    expected_compiler: crate::PairedCompilerFingerprintV1,
    candidate: &PreparedArtifactCandidate,
    context: &BuildContext,
) -> Option<crate::TrustedCoreSlotReceiptV1> {
    let snapshot = ImmutableInputSnapshot::capture_no_follow(
        receipt_slot,
        context.limits.artifact_decode().owned_bytes,
    )
    .ok()?;
    let (receipt, _) = crate::decode_trusted_core_slot_receipt_v1(
        snapshot.as_bytes(),
        context.limits.artifact_decode(),
    )
    .ok()?;
    let summary = candidate.summary();
    if !core_receipt_matches(
        &receipt,
        expected_source_key,
        expected_compiler,
        summary.artifact_fingerprint(),
        context.target.lir_target_selection(),
        summary.profile(),
    ) {
        return None;
    }
    Some(receipt)
}

fn core_receipt_matches(
    receipt: &crate::TrustedCoreSlotReceiptV1,
    expected_source_key: crate::CoreSourceSnapshotKeyV1,
    expected_compiler: crate::PairedCompilerFingerprintV1,
    expected_artifact: scoop_slib::ArtifactFingerprint,
    expected_target: scoop_lir::ValidatedLirTargetSelection,
    actual_profile: &ArtifactCapabilityProfileId,
) -> bool {
    let body = receipt.body();
    let expected_profile = ArtifactCapabilityProfileId::single_cone_strong();
    body.source_snapshot_key() == expected_source_key
        && body.artifact_fingerprint().matches(expected_artifact)
        && body.target_selection().selection() == expected_target
        && body.compiler() == expected_compiler
        && body.artifact_profile() == &expected_profile
        && actual_profile == &expected_profile
        && crate::validate_warning_origins(body.structured_warnings(), &[ConeIdentity::CORE])
            .is_ok()
}

fn digest_from_source(digest: SourceContentDigest) -> Digest256 {
    Digest256::from_array(*digest.as_array())
}

#[derive(Debug)]
struct CoreSlotLock {
    file: File,
    path: PathBuf,
}

impl CoreSlotLock {
    fn acquire(
        layout: &scoop_toolchain::TrustedCoreSlotLayoutV1,
    ) -> Result<Self, PrepareBuildGraphError> {
        std::fs::create_dir_all(layout.artifact_root()).map_err(|source| {
            PrepareBuildGraphError::CoreLockIo {
                operation: CoreLockOperation::CreateDirectory,
                path: layout.artifact_root().to_path_buf(),
                source,
            }
        })?;
        match std::fs::symlink_metadata(layout.lock()) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(PrepareBuildGraphError::InvalidCoreLockFileType(
                    layout.lock().to_path_buf(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(PrepareBuildGraphError::CoreLockIo {
                    operation: CoreLockOperation::Inspect,
                    path: layout.lock().to_path_buf(),
                    source,
                });
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(layout.lock())
            .map_err(|source| PrepareBuildGraphError::CoreLockIo {
                operation: CoreLockOperation::Open,
                path: layout.lock().to_path_buf(),
                source,
            })?;
        FileExt::lock(&file).map_err(|source| PrepareBuildGraphError::CoreLockIo {
            operation: CoreLockOperation::Lock,
            path: layout.lock().to_path_buf(),
            source,
        })?;
        let opened = file
            .metadata()
            .map_err(|source| PrepareBuildGraphError::CoreLockIo {
                operation: CoreLockOperation::Inspect,
                path: layout.lock().to_path_buf(),
                source,
            })?;
        let visible = std::fs::symlink_metadata(layout.lock()).map_err(|source| {
            PrepareBuildGraphError::CoreLockIo {
                operation: CoreLockOperation::Inspect,
                path: layout.lock().to_path_buf(),
                source,
            }
        })?;
        if !visible.file_type().is_file() || !same_file_identity(&opened, &visible) {
            return Err(PrepareBuildGraphError::CoreLockPathChanged(
                layout.lock().to_path_buf(),
            ));
        }
        Ok(Self {
            file,
            path: layout.lock().to_path_buf(),
        })
    }
}

#[cfg(unix)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(_left: &std::fs::Metadata, _right: &std::fs::Metadata) -> bool {
    true
}

impl Drop for CoreSlotLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests;
