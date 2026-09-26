use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use scoop_identity::{ArtifactCapabilityProfileId, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_protocol::{DiagnosticOriginV1, ProtocolConeIdentity, StructuredDiagnosticV1};
use scoop_slib::{
    ArtifactFingerprint, ArtifactManifestSummaryError, ArtifactSnapshot, ConeRecord,
    DependencyRecord,
};

use super::{CacheReceiptBodyV1, ConeCompileCacheKeyV1, RawCompileCacheEntryV1};
use crate::artifact::{
    ArtifactClosurePlan, ArtifactClosureValidationError, BuildArtifact, CompletedNode,
    ValidatedArtifactClosure,
};
use crate::{CompileCacheKeyError, PairedCompilerFingerprintV1, StagingError};

pub(crate) struct ValidatedCacheHitV1 {
    identity: ConeIdentity,
    artifact: Rc<BuildArtifact>,
    closures: ValidatedArtifactClosure,
    warnings: Vec<StructuredDiagnosticV1>,
}

impl ValidatedCacheHitV1 {
    pub(crate) const fn artifact(&self) -> &Rc<BuildArtifact> {
        &self.artifact
    }

    pub(crate) fn into_completed(self, materialized_path: PathBuf) -> CompletedNode {
        CompletedNode::from_cache_hit(
            self.identity,
            self.artifact,
            self.closures,
            materialized_path,
            self.warnings,
        )
    }
}

struct ActualCacheBinding<'actual> {
    key: ConeCompileCacheKeyV1,
    artifact: ArtifactFingerprint,
    cone: &'actual ConeRecord,
    target: ValidatedLirTargetSelection,
    direct_dependencies: &'actual [DependencyRecord],
    compiler: PairedCompilerFingerprintV1,
    profile: &'actual ArtifactCapabilityProfileId,
}

pub(crate) fn validate_cache_entry(
    plan: &ArtifactClosurePlan,
    identity: ConeIdentity,
    expected_key: ConeCompileCacheKeyV1,
    entry: RawCompileCacheEntryV1,
    completed: &[&CompletedNode],
    compiler: PairedCompilerFingerprintV1,

    target: ValidatedLirTargetSelection,
) -> Result<ValidatedCacheHitV1, CacheCompletionError> {
    if entry.key() != expected_key {
        return Err(CacheCompletionError::EntryKeyMismatch {
            expected: expected_key,
            actual: entry.key(),
        });
    }
    let mut artifacts = BTreeMap::new();
    for node in completed {
        if node.cone() == identity {
            return Err(CacheCompletionError::CurrentNodeAlreadyCompleted(identity));
        }
        if artifacts
            .insert(node.cone(), node.shared_artifact())
            .is_some()
        {
            return Err(CacheCompletionError::DuplicateCompletedNode(node.cone()));
        }
    }
    let artifact_snapshot = Arc::new(ArtifactSnapshot::from_shared(
        entry.artifact().shared_bytes(),
    ));
    let artifact = BuildArtifact::read(artifact_snapshot, target)
        .map_err(|source| CacheCompletionError::Artifact(Box::new(source)))?;
    artifacts.insert(identity, Rc::clone(&artifact));
    let closures = plan
        .validate(identity, &artifacts)
        .map_err(|source| CacheCompletionError::Plan(Box::new(source)))?;

    let summary = artifact.summary();
    validate_receipt_binding(
        entry.receipt().body(),
        &ActualCacheBinding {
            key: expected_key,
            artifact: summary.artifact_fingerprint(),
            cone: summary.cone(),
            target: summary.target_selection(),
            direct_dependencies: summary.direct_dependencies(),
            compiler,
            profile: summary.profile(),
        },
    )
    .map_err(CacheCompletionError::ReceiptBinding)?;
    validate_warning_origins(
        entry.receipt().body().structured_warnings(),
        closures.dependency_first(),
    )?;
    Ok(ValidatedCacheHitV1 {
        identity,
        artifact,
        closures,
        warnings: entry.receipt().body().structured_warnings().to_vec(),
    })
}

pub(crate) fn validate_warning_origins(
    warnings: &[StructuredDiagnosticV1],
    closure: &[ConeIdentity],
) -> Result<(), CacheCompletionError> {
    for warning in warnings {
        validate_warning_origin(warning.code(), warning.origin(), closure)?;
        for note in warning.notes() {
            validate_warning_origin(warning.code(), note.origin(), closure)?;
        }
    }
    Ok(())
}

fn validate_warning_origin(
    code: &str,
    origin: &DiagnosticOriginV1,
    closure: &[ConeIdentity],
) -> Result<(), CacheCompletionError> {
    match origin {
        DiagnosticOriginV1::None => Ok(()),
        DiagnosticOriginV1::SemanticSourceSpan { cone, .. }
            if closure
                .iter()
                .any(|identity| identity.as_array() == cone.as_array()) =>
        {
            Ok(())
        }
        DiagnosticOriginV1::SemanticSourceSpan { cone, .. } => {
            Err(CacheCompletionError::WarningOriginOutsideClosure {
                code: code.to_owned(),
                cone: *cone,
            })
        }
        DiagnosticOriginV1::HostPathSpan { .. } | DiagnosticOriginV1::ArtifactPath { .. } => Err(
            CacheCompletionError::PersistedHostPathWarning(code.to_owned()),
        ),
    }
}

fn validate_receipt_binding(
    receipt: &CacheReceiptBodyV1,
    actual: &ActualCacheBinding<'_>,
) -> Result<(), CacheReceiptBindingError> {
    if receipt.cache_key() != actual.key {
        return Err(CacheReceiptBindingError::CacheKey);
    }
    if !receipt.artifact_fingerprint().matches(actual.artifact) {
        return Err(CacheReceiptBindingError::ArtifactFingerprint);
    }
    if receipt.cone() != actual.cone {
        return Err(CacheReceiptBindingError::Cone);
    }
    if receipt.target_selection().selection() != actual.target {
        return Err(CacheReceiptBindingError::Target);
    }
    let mut actual_dependencies = actual.direct_dependencies.iter().collect::<Vec<_>>();
    actual_dependencies.sort_by(|left, right| {
        crate::discovery::compare_coordinates(left.coordinate(), right.coordinate())
    });
    if !receipt.direct_dependencies().iter().eq(actual_dependencies) {
        return Err(CacheReceiptBindingError::DirectDependencies);
    }
    if receipt.compiler() != actual.compiler {
        return Err(CacheReceiptBindingError::Compiler);
    }
    if receipt.artifact_profile() != actual.profile {
        return Err(CacheReceiptBindingError::ArtifactProfile);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheReceiptBindingError {
    CacheKey,
    ArtifactFingerprint,
    Cone,
    Target,
    DirectDependencies,
    Compiler,
    ArtifactProfile,
}

impl fmt::Display for CacheReceiptBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CacheKey => "cache receipt names a different cache key",
            Self::ArtifactFingerprint => "cache receipt names a different artifact fingerprint",
            Self::Cone => "cache receipt names a different Cone record",
            Self::Target => "cache receipt names a different target selection",
            Self::DirectDependencies => "cache receipt names different direct dependencies",
            Self::Compiler => "cache receipt names a different paired compiler",
            Self::ArtifactProfile => "cache receipt names a different artifact profile",
        })
    }
}

impl std::error::Error for CacheReceiptBindingError {}

#[derive(Debug)]
pub enum CacheCompletionError {
    NotOrdinarySource(ConeIdentity),
    CacheKey(Box<CompileCacheKeyError>),

    EntryKeyMismatch {
        expected: ConeCompileCacheKeyV1,
        actual: ConeCompileCacheKeyV1,
    },
    CurrentNodeAlreadyCompleted(ConeIdentity),
    DuplicateCompletedNode(ConeIdentity),
    Artifact(Box<ArtifactManifestSummaryError>),
    Plan(Box<ArtifactClosureValidationError>),
    ReceiptBinding(CacheReceiptBindingError),
    WarningOriginOutsideClosure {
        code: String,
        cone: ProtocolConeIdentity,
    },
    PersistedHostPathWarning(String),
    Staging(StagingError),
}

impl fmt::Display for CacheCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOrdinarySource(identity) => {
                write!(
                    formatter,
                    "Cone {identity} is not an ordinary cacheable source node"
                )
            }
            Self::CacheKey(source) => {
                write!(formatter, "cannot derive compile cache key: {source}")
            }

            Self::EntryKeyMismatch { expected, actual } => write!(
                formatter,
                "cache entry key mismatch: expected {expected}, found {actual}"
            ),
            Self::CurrentNodeAlreadyCompleted(identity) => {
                write!(
                    formatter,
                    "cache candidate Cone {identity} is already completed"
                )
            }
            Self::DuplicateCompletedNode(identity) => {
                write!(formatter, "completed input repeats Cone {identity}")
            }
            Self::Artifact(source) => {
                write!(
                    formatter,
                    "cached artifact failed archive validation: {source}"
                )
            }
            Self::Plan(source) => {
                write!(
                    formatter,
                    "cached artifact does not match the resolved graph: {source}"
                )
            }
            Self::ReceiptBinding(source) => source.fmt(formatter),
            Self::WarningOriginOutsideClosure { code, cone } => write!(
                formatter,
                "cached warning {code} refers to Cone {} outside the validated closure",
                hex(cone.as_array())
            ),
            Self::PersistedHostPathWarning(code) => {
                write!(
                    formatter,
                    "cached warning {code} retains a host-path origin"
                )
            }
            Self::Staging(source) => write!(formatter, "cannot materialize cache hit: {source}"),
        }
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        write!(text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

impl std::error::Error for CacheCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CacheKey(source) => Some(source),

            Self::Artifact(source) => Some(source),
            Self::Plan(source) => Some(source),
            Self::ReceiptBinding(source) => Some(source),
            Self::Staging(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
