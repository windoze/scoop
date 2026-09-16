use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_identity::ConeIdentity;
use scoop_lir::{CBridgeToolchainProfileV1, ValidatedLirTargetSelection};
use scoop_protocol::StructuredDiagnosticV1;
use scoop_slib::{
    ArtifactFingerprint, CanonicalDefinedLinkSymbolOwnerSetV1, CompileArtifactPurpose,
    DependencyRecord, DualValidatedArtifactError, DualValidatedArtifactHandle,
    DualValidatedArtifactReopenError, LinkArtifactPurpose, SemanticFingerprintRecord,
    SlibClosureDecodeMeterV1,
};
use scoop_wire::DecodeLimits;

use super::{
    ArtifactClosurePlan, ArtifactClosureValidationError, ValidatedArtifactClosure,
    ValidatedDualArtifactClosure,
};
use crate::{
    CacheCompletionError, CoreSourceSnapshotKeyV1, PairedCompilerFingerprintV1,
    PreparedArtifactCandidate, TrustedCoreSlotReceiptV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletedNodeOrigin {
    Prebuilt,
    CacheHit,
    Compiled,
    TrustedCore,
}

/// A transport path under the current build's private staging area. The path
/// is never used as semantic identity or artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateArtifactPath(PathBuf);

impl PrivateArtifactPath {
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub(crate) const fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

/// A node is complete only after its artifact and both purpose closures pass
/// the parent-side validation gate.
#[derive(Clone, Debug)]
pub struct CompletedNode {
    cone: ConeIdentity,
    origin: CompletedNodeOrigin,
    artifact: Arc<DualValidatedArtifactHandle>,
    compile_closure: ValidatedArtifactClosure<CompileArtifactPurpose>,
    link_closure: ValidatedArtifactClosure<LinkArtifactPurpose>,
    materialized_child_path: PrivateArtifactPath,
    warnings: Vec<StructuredDiagnosticV1>,
}

impl CompletedNode {
    pub const fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub const fn origin(&self) -> CompletedNodeOrigin {
        self.origin
    }

    pub fn artifact(&self) -> &DualValidatedArtifactHandle {
        &self.artifact
    }

    pub const fn compile_closure(&self) -> &ValidatedArtifactClosure<CompileArtifactPurpose> {
        &self.compile_closure
    }

    pub const fn link_closure(&self) -> &ValidatedArtifactClosure<LinkArtifactPurpose> {
        &self.link_closure
    }

    pub const fn materialized_child_path(&self) -> &PrivateArtifactPath {
        &self.materialized_child_path
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub(crate) fn shared_artifact(&self) -> Arc<DualValidatedArtifactHandle> {
        Arc::clone(&self.artifact)
    }

    pub(crate) fn replace_warnings(&mut self, warnings: Vec<StructuredDiagnosticV1>) {
        self.warnings = warnings;
    }

    pub(crate) fn from_cache_hit(
        cone: ConeIdentity,
        artifact: Arc<DualValidatedArtifactHandle>,
        closures: ValidatedDualArtifactClosure,
        materialized_child_path: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        let (compile_closure, link_closure) = closures.into_parts();
        Self {
            cone,
            origin: CompletedNodeOrigin::CacheHit,
            artifact,
            compile_closure,
            link_closure,
            materialized_child_path: PrivateArtifactPath::new(materialized_child_path),
            warnings,
        }
    }

    fn from_trusted_core(
        artifact: Arc<DualValidatedArtifactHandle>,
        closures: ValidatedDualArtifactClosure,
        materialized_child_path: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        let (compile_closure, link_closure) = closures.into_parts();
        Self {
            cone: ConeIdentity::CORE,
            origin: CompletedNodeOrigin::TrustedCore,
            artifact,
            compile_closure,
            link_closure,
            materialized_child_path: PrivateArtifactPath::new(materialized_child_path),
            warnings,
        }
    }

    fn from_compiled(
        cone: ConeIdentity,
        artifact: Arc<DualValidatedArtifactHandle>,
        closures: ValidatedDualArtifactClosure,
        materialized_child_path: PathBuf,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Self {
        let (compile_closure, link_closure) = closures.into_parts();
        Self {
            cone,
            origin: CompletedNodeOrigin::Compiled,
            artifact,
            compile_closure,
            link_closure,
            materialized_child_path: PrivateArtifactPath::new(materialized_child_path),
            warnings,
        }
    }
}

#[derive(Debug)]
pub enum CompiledCompletionError {
    CurrentNodeAlreadyCompleted(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    MissingTrustedCore,
    TrustedCoreReopen(DualValidatedArtifactReopenError),
    Artifact(Box<DualValidatedArtifactError>),
    Plan(Box<ArtifactClosureValidationError>),
    Warnings(Box<CacheCompletionError>),
}

impl fmt::Display for CompiledCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentNodeAlreadyCompleted(identity) => {
                write!(formatter, "compiled Cone {identity} is already completed")
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::MissingTrustedCore => {
                formatter.write_str("compiled artifact validation requires completed trusted core")
            }
            Self::TrustedCoreReopen(source) => write!(
                formatter,
                "cannot reopen completed trusted core Link view: {source}"
            ),
            Self::Artifact(source) => {
                write!(
                    formatter,
                    "compiled artifact failed dual-view validation: {source}"
                )
            }
            Self::Plan(source) => {
                write!(
                    formatter,
                    "compiled artifact does not match the resolved graph: {source}"
                )
            }
            Self::Warnings(source) => {
                write!(formatter, "compiled warnings are not persistable: {source}")
            }
        }
    }
}

impl std::error::Error for CompiledCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::TrustedCoreReopen(source) => source,
            Self::Artifact(source) => source.as_ref(),
            Self::Plan(source) => source.as_ref(),
            Self::Warnings(source) => source.as_ref(),
            Self::CurrentNodeAlreadyCompleted(_)
            | Self::DuplicateCompletedNode(_)
            | Self::MissingTrustedCore => return None,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_compiled_candidate(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    snapshot: Arc<scoop_slib::ArtifactSnapshot>,
    materialized_path: PathBuf,
    completed: &[&CompletedNode],
    warnings: Vec<StructuredDiagnosticV1>,
    limits: DecodeLimits,
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
    meter: &mut SlibClosureDecodeMeterV1,
) -> Result<CompletedNode, CompiledCompletionError> {
    let mut artifacts = BTreeMap::new();
    for node in completed {
        if node.cone == identity {
            return Err(CompiledCompletionError::CurrentNodeAlreadyCompleted(
                identity,
            ));
        }
        if artifacts
            .insert(node.cone, node.shared_artifact())
            .is_some()
        {
            return Err(CompiledCompletionError::DuplicateCompletedNode(node.cone));
        }
    }
    let core = artifacts
        .get(&ConeIdentity::CORE)
        .ok_or(CompiledCompletionError::MissingTrustedCore)?;
    let core_owners: CanonicalDefinedLinkSymbolOwnerSetV1 = core
        .with_link_view(|view| view.link_identity_closure().defined_symbols().clone())
        .map_err(CompiledCompletionError::TrustedCoreReopen)?;
    let artifact = Arc::new(
        DualValidatedArtifactHandle::validate(
            snapshot,
            limits,
            target,
            &core_owners,
            c_bridge_profile,
            meter,
        )
        .map_err(|source| CompiledCompletionError::Artifact(Box::new(source)))?,
    );
    artifacts.insert(identity, Arc::clone(&artifact));
    let closures = plan
        .validate(identity, &artifacts)
        .map_err(|source| CompiledCompletionError::Plan(Box::new(source)))?;
    crate::validate_warning_origins(&warnings, closures.compile().dependency_first())
        .map_err(|source| CompiledCompletionError::Warnings(Box::new(source)))?;
    Ok(CompletedNode::from_compiled(
        identity,
        artifact,
        closures,
        materialized_path,
        warnings,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedCoreReceiptBindingField {
    SourceSnapshotKey,
    ArtifactFingerprint,
    Target,
    Compiler,
    ArtifactProfile,
    Warnings,
}

impl fmt::Display for TrustedCoreReceiptBindingField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SourceSnapshotKey => "source snapshot key",
            Self::ArtifactFingerprint => "artifact fingerprint",
            Self::Target => "target selection",
            Self::Compiler => "paired compiler",
            Self::ArtifactProfile => "artifact profile",
            Self::Warnings => "structured warning origins",
        })
    }
}

#[derive(Debug)]
pub enum TrustedCoreCompletionError {
    BootstrapRequired,
    MissingPreparedCandidate,
    MissingPreparedReceipt,
    Artifact(Box<DualValidatedArtifactError>),
    Plan(Box<ArtifactClosureValidationError>),
    ReceiptBinding(TrustedCoreReceiptBindingField),
}

impl fmt::Display for TrustedCoreCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BootstrapRequired => {
                formatter.write_str("trusted core slot is not prepared for reuse")
            }
            Self::MissingPreparedCandidate => {
                formatter.write_str("reusable trusted core has no immutable artifact candidate")
            }
            Self::MissingPreparedReceipt => {
                formatter.write_str("reusable trusted core has no validated slot receipt")
            }
            Self::Artifact(source) => {
                write!(
                    formatter,
                    "trusted core failed dual-view validation: {source}"
                )
            }
            Self::Plan(source) => {
                write!(
                    formatter,
                    "trusted core does not match the resolved graph: {source}"
                )
            }
            Self::ReceiptBinding(field) => {
                write!(formatter, "trusted core receipt has stale {field}")
            }
        }
    }
}

impl std::error::Error for TrustedCoreCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(source) => Some(source),
            Self::Plan(source) => Some(source),
            _ => None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_trusted_core_candidate(
    plan: &ArtifactClosurePlan,
    candidate: PreparedArtifactCandidate,
    receipt: TrustedCoreSlotReceiptV1,
    source_key: CoreSourceSnapshotKeyV1,
    compiler: PairedCompilerFingerprintV1,
    limits: DecodeLimits,
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
    meter: &mut SlibClosureDecodeMeterV1,
) -> Result<CompletedNode, TrustedCoreCompletionError> {
    let artifact = Arc::new(
        DualValidatedArtifactHandle::validate(
            Arc::clone(candidate.snapshot()),
            limits,
            target,
            &CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap(),
            c_bridge_profile,
            meter,
        )
        .map_err(|source| TrustedCoreCompletionError::Artifact(Box::new(source)))?,
    );
    let artifacts = BTreeMap::from([(ConeIdentity::CORE, Arc::clone(&artifact))]);
    let closures = plan
        .validate(ConeIdentity::CORE, &artifacts)
        .map_err(|source| TrustedCoreCompletionError::Plan(Box::new(source)))?;
    validate_trusted_core_receipt(&receipt, source_key, compiler, artifact.as_ref(), target)?;
    Ok(CompletedNode::from_trusted_core(
        artifact,
        closures,
        candidate.materialized_path().to_path_buf(),
        receipt.body().structured_warnings().to_vec(),
    ))
}

fn validate_trusted_core_receipt(
    receipt: &TrustedCoreSlotReceiptV1,
    source_key: CoreSourceSnapshotKeyV1,
    compiler: PairedCompilerFingerprintV1,
    artifact: &DualValidatedArtifactHandle,
    target: ValidatedLirTargetSelection,
) -> Result<(), TrustedCoreCompletionError> {
    let body = receipt.body();
    let publication = artifact.publication();
    if body.source_snapshot_key() != source_key {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::SourceSnapshotKey,
        ));
    }
    if !body
        .artifact_fingerprint()
        .matches(publication.artifact_fingerprint())
    {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::ArtifactFingerprint,
        ));
    }
    if body.target_selection().selection() != target || publication.target_selection() != target {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::Target,
        ));
    }
    if body.compiler() != compiler {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::Compiler,
        ));
    }
    if body.artifact_profile() != publication.profile() {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::ArtifactProfile,
        ));
    }
    if crate::validate_warning_origins(body.structured_warnings(), &[ConeIdentity::CORE]).is_err() {
        return Err(TrustedCoreCompletionError::ReceiptBinding(
            TrustedCoreReceiptBindingField::Warnings,
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum PrebuiltCompletionError {
    NotPrebuilt(ConeIdentity),
    EmptyCandidateSet(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    CurrentNodeAlreadyCompleted(ConeIdentity),
    MissingTrustedCore,
    TrustedCoreReopen(DualValidatedArtifactReopenError),
    CandidateArtifact {
        path: PathBuf,
        source: Box<DualValidatedArtifactError>,
    },
    CandidatePlan {
        path: PathBuf,
        source: Box<ArtifactClosureValidationError>,
    },
    StalePrebuiltDependency {
        path: PathBuf,
        dependent: ConeIdentity,
        dependency: ConeIdentity,
        recorded: Box<DependencyRecord>,
        completed: Box<DependencyRecord>,
    },
    CandidateDisagreement {
        first: PathBuf,
        second: PathBuf,
    },
}

impl fmt::Display for PrebuiltCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPrebuilt(identity) => {
                write!(formatter, "Cone {identity} is not a prepared prebuilt node")
            }
            Self::EmptyCandidateSet(identity) => {
                write!(
                    formatter,
                    "prepared prebuilt Cone {identity} has no candidates"
                )
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::CurrentNodeAlreadyCompleted(identity) => write!(
                formatter,
                "prebuilt Cone {identity} was supplied as its own completed dependency"
            ),
            Self::MissingTrustedCore => {
                formatter.write_str("prebuilt validation requires completed trusted core")
            }
            Self::TrustedCoreReopen(error) => {
                write!(
                    formatter,
                    "cannot reopen completed trusted core Link view: {error}"
                )
            }
            Self::CandidateArtifact { path, source } => write!(
                formatter,
                "prebuilt candidate {} failed dual-view validation: {source}",
                path.display()
            ),
            Self::CandidatePlan { path, source } => write!(
                formatter,
                "prebuilt candidate {} does not match the resolved graph: {source}",
                path.display()
            ),
            Self::StalePrebuiltDependency {
                path,
                dependent,
                dependency,
                ..
            } => write!(
                formatter,
                "prebuilt candidate {} for {dependent} records stale semantic fingerprints for dependency {dependency}",
                path.display()
            ),
            Self::CandidateDisagreement { first, second } => write!(
                formatter,
                "prebuilt candidates {} and {} disagree after full validation",
                first.display(),
                second.display()
            ),
        }
    }
}

impl std::error::Error for PrebuiltCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TrustedCoreReopen(error) => Some(error),
            Self::CandidateArtifact { source, .. } => Some(source),
            Self::CandidatePlan { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateAgreement {
    artifact: ArtifactFingerprint,
    compile_semantic: SemanticFingerprintRecord,
    link_semantic: SemanticFingerprintRecord,
}

struct ValidatedCandidate {
    source_locator: PathBuf,
    materialized_path: PathBuf,
    artifact: Arc<DualValidatedArtifactHandle>,
    closures: ValidatedDualArtifactClosure,
    agreement: CandidateAgreement,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_prebuilt_candidates(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    candidates: Vec<PreparedArtifactCandidate>,
    completed: &[&CompletedNode],
    limits: DecodeLimits,
    target: ValidatedLirTargetSelection,
    c_bridge_profile: &CBridgeToolchainProfileV1,
    meter: &mut SlibClosureDecodeMeterV1,
) -> Result<CompletedNode, PrebuiltCompletionError> {
    let mut artifacts = BTreeMap::new();
    for node in completed {
        if node.cone == identity {
            return Err(PrebuiltCompletionError::CurrentNodeAlreadyCompleted(
                identity,
            ));
        }
        if artifacts
            .insert(node.cone, node.shared_artifact())
            .is_some()
        {
            return Err(PrebuiltCompletionError::DuplicateCompletedNode(node.cone));
        }
    }
    let core = artifacts
        .get(&ConeIdentity::CORE)
        .ok_or(PrebuiltCompletionError::MissingTrustedCore)?;
    let core_owners: CanonicalDefinedLinkSymbolOwnerSetV1 = core
        .with_link_view(|view| view.link_identity_closure().defined_symbols().clone())
        .map_err(PrebuiltCompletionError::TrustedCoreReopen)?;

    let mut validated = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let artifact = Arc::new(
            DualValidatedArtifactHandle::validate(
                Arc::clone(candidate.snapshot()),
                limits,
                target,
                &core_owners,
                c_bridge_profile,
                meter,
            )
            .map_err(|source| PrebuiltCompletionError::CandidateArtifact {
                path: candidate.source_locator().to_path_buf(),
                source: Box::new(source),
            })?,
        );
        artifacts.insert(identity, Arc::clone(&artifact));
        let closures = plan
            .validate(identity, &artifacts)
            .map_err(|source| match source {
                ArtifactClosureValidationError::StaleDependency {
                    dependent,
                    dependency,
                    recorded,
                    completed,
                } => PrebuiltCompletionError::StalePrebuiltDependency {
                    path: candidate.source_locator().to_path_buf(),
                    dependent,
                    dependency,
                    recorded,
                    completed,
                },
                source => PrebuiltCompletionError::CandidatePlan {
                    path: candidate.source_locator().to_path_buf(),
                    source: Box::new(source),
                },
            })?;
        let publication = artifact.publication();
        validated.push(ValidatedCandidate {
            source_locator: candidate.source_locator().to_path_buf(),
            materialized_path: candidate.materialized_path().to_path_buf(),
            agreement: CandidateAgreement {
                artifact: publication.artifact_fingerprint(),
                compile_semantic: publication.compile_summary().semantic_fingerprints(),
                link_semantic: publication.link_summary().semantic_fingerprints(),
            },
            artifact,
            closures,
        });
    }

    let first = validated
        .first()
        .ok_or(PrebuiltCompletionError::EmptyCandidateSet(identity))?;
    if let Some(disagreeing) = validated
        .iter()
        .skip(1)
        .find(|candidate| candidate.agreement != first.agreement)
    {
        return Err(PrebuiltCompletionError::CandidateDisagreement {
            first: first.source_locator.clone(),
            second: disagreeing.source_locator.clone(),
        });
    }
    let selected = validated
        .into_iter()
        .min_by(|left, right| left.source_locator.cmp(&right.source_locator))
        .ok_or(PrebuiltCompletionError::EmptyCandidateSet(identity))?;
    let (compile_closure, link_closure) = selected.closures.into_parts();
    Ok(CompletedNode {
        cone: identity,
        origin: CompletedNodeOrigin::Prebuilt,
        artifact: selected.artifact,
        compile_closure,
        link_closure,
        materialized_child_path: PrivateArtifactPath::new(selected.materialized_path),
        warnings: Vec::new(),
    })
}
