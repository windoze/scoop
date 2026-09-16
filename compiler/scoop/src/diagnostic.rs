//! Stable phase and code classification for orchestration failures.

use std::fmt;

use scoop_protocol::StructuredDiagnosticV1;

use crate::{
    BuildGraphDiscoveryError, BuildGraphExecutionError, BuildGraphRequestError,
    CacheCompletionError, CacheIoOperation, ChildTransportError, CompileCacheStoreError,
    CoreBootstrapExecutionError, DependencyLocatorError, LoadBuildRootError,
    OrdinarySourceExecutionError, PrebuiltCompletionError, PrepareBuildGraphError,
    ResolveBuildGraphError,
};

/// Canonical order in which build orchestration failures are reported.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuildFailurePhase {
    Request,
    Toolchain,
    Root,
    Locator,
    Summary,
    GraphIdentity,
    GraphVersionKind,
    GraphCycleOrder,
    SourceSnapshot,
    PrebuiltArtifact,
    CoreSlot,
    Cache,
    ChildTransport,
    ChildDiagnostic,
    ChildOutput,
    CachePublish,
}

impl BuildFailurePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "Request",
            Self::Toolchain => "Toolchain",
            Self::Root => "Root",
            Self::Locator => "Locator",
            Self::Summary => "Summary",
            Self::GraphIdentity => "GraphIdentity",
            Self::GraphVersionKind => "GraphVersionKind",
            Self::GraphCycleOrder => "GraphCycleOrder",
            Self::SourceSnapshot => "SourceSnapshot",
            Self::PrebuiltArtifact => "PrebuiltArtifact",
            Self::CoreSlot => "CoreSlot",
            Self::Cache => "Cache",
            Self::ChildTransport => "ChildTransport",
            Self::ChildDiagnostic => "ChildDiagnostic",
            Self::ChildOutput => "ChildOutput",
            Self::CachePublish => "CachePublish",
        }
    }
}

impl fmt::Display for BuildFailurePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A stable orchestrator-owned diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BuildDiagnosticCode(&'static str);

impl BuildDiagnosticCode {
    pub const LOCATOR_NOT_FOUND: Self = Self("SCOOP_LOCATOR_NOT_FOUND");
    pub const LOCATOR_WRONG_TYPE: Self = Self("SCOOP_LOCATOR_WRONG_TYPE");
    pub const LOCATOR_COORDINATE_MISMATCH: Self = Self("SCOOP_LOCATOR_COORDINATE_MISMATCH");
    pub const LOCATOR_CONFLICTING_SOURCE: Self = Self("SCOOP_LOCATOR_CONFLICTING_SOURCE");
    pub const LOCATOR_CONFLICTING_REPRESENTATION: Self =
        Self("SCOOP_LOCATOR_CONFLICTING_REPRESENTATION");
    pub const LOCATOR_AMBIGUOUS_ARTIFACT: Self = Self("SCOOP_LOCATOR_AMBIGUOUS_ARTIFACT");

    pub const GRAPH_RESERVED_IDENTITY: Self = Self("SCOOP_GRAPH_RESERVED_IDENTITY");
    pub const GRAPH_MULTIPLE_VERSIONS: Self = Self("SCOOP_GRAPH_MULTIPLE_VERSIONS");
    pub const GRAPH_EXECUTABLE_DEPENDENCY: Self = Self("SCOOP_GRAPH_EXECUTABLE_DEPENDENCY");
    pub const GRAPH_SINGLE_FILE_DEPENDENCY: Self = Self("SCOOP_GRAPH_SINGLE_FILE_DEPENDENCY");
    pub const GRAPH_MISSING_CORE: Self = Self("SCOOP_GRAPH_MISSING_CORE");
    pub const GRAPH_CYCLE: Self = Self("SCOOP_GRAPH_CYCLE");
    pub const GRAPH_UNREACHABLE_NODE: Self = Self("SCOOP_GRAPH_UNREACHABLE_NODE");
    pub const GRAPH_RESOURCE_LIMIT: Self = Self("SCOOP_GRAPH_RESOURCE_LIMIT");

    pub const PREBUILT_SUMMARY_MISMATCH: Self = Self("SCOOP_PREBUILT_SUMMARY_MISMATCH");
    pub const PREBUILT_VIEW_INVALID: Self = Self("SCOOP_PREBUILT_VIEW_INVALID");
    pub const PREBUILT_STALE_DEPENDENCY: Self = Self("SCOOP_PREBUILT_STALE_DEPENDENCY");
    pub const PREBUILT_CHANGED: Self = Self("SCOOP_PREBUILT_CHANGED");

    pub const CACHE_IO: Self = Self("SCOOP_CACHE_IO");
    pub const CACHE_ENTRY_CORRUPT: Self = Self("SCOOP_CACHE_ENTRY_CORRUPT");
    pub const CACHE_LOCK: Self = Self("SCOOP_CACHE_LOCK");
    pub const CACHE_NONDETERMINISTIC_PRODUCTION: Self =
        Self("SCOOP_CACHE_NONDETERMINISTIC_PRODUCTION");
    pub const CACHE_PUBLISH: Self = Self("SCOOP_CACHE_PUBLISH");

    pub const CORE_SLOT_CORRUPT: Self = Self("SCOOP_CORE_SLOT_CORRUPT");
    pub const CORE_SOURCE_CHANGED: Self = Self("SCOOP_CORE_SOURCE_CHANGED");
    pub const CORE_BOOTSTRAP_FAILED: Self = Self("SCOOP_CORE_BOOTSTRAP_FAILED");

    pub const CHILD_TOOL_MISMATCH: Self = Self("SCOOP_CHILD_TOOL_MISMATCH");
    pub const CHILD_TRANSPORT: Self = Self("SCOOP_CHILD_TRANSPORT");
    pub const CHILD_PROTOCOL: Self = Self("SCOOP_CHILD_PROTOCOL");
    pub const CHILD_EXIT: Self = Self("SCOOP_CHILD_EXIT");
    pub const CHILD_OUTPUT_MISSING: Self = Self("SCOOP_CHILD_OUTPUT_MISSING");
    pub const CHILD_OUTPUT_PLAN_MISMATCH: Self = Self("SCOOP_CHILD_OUTPUT_PLAN_MISMATCH");
    pub const CHILD_RESPONSE_MISMATCH: Self = Self("SCOOP_CHILD_RESPONSE_MISMATCH");

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for BuildDiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Stable outer context for an orchestration failure.
///
/// `code` is absent when the frozen code family has no more specific member.
/// It is always absent when the child supplied structured diagnostics, whose
/// original codes must be preserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildFailureClassification {
    phase: BuildFailurePhase,
    code: Option<BuildDiagnosticCode>,
}

impl BuildFailureClassification {
    pub const fn new(phase: BuildFailurePhase, code: Option<BuildDiagnosticCode>) -> Self {
        Self { phase, code }
    }

    pub const fn phase(self) -> BuildFailurePhase {
        self.phase
    }

    pub const fn code(self) -> Option<BuildDiagnosticCode> {
        self.code
    }
}

/// Adds stable orchestrator context without flattening nested error details.
pub trait ClassifyBuildFailure {
    fn classification(&self) -> BuildFailureClassification;
}

const fn classified(
    phase: BuildFailurePhase,
    code: BuildDiagnosticCode,
) -> BuildFailureClassification {
    BuildFailureClassification::new(phase, Some(code))
}

const fn phase_only(phase: BuildFailurePhase) -> BuildFailureClassification {
    BuildFailureClassification::new(phase, None)
}

impl ClassifyBuildFailure for BuildGraphRequestError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::TooManyArtifactSearchRoots { .. } => classified(
                BuildFailurePhase::Request,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::Toolchain(_) => classified(
                BuildFailurePhase::Toolchain,
                BuildDiagnosticCode::CHILD_TOOL_MISMATCH,
            ),
        }
    }
}

impl ClassifyBuildFailure for LoadBuildRootError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::RootManifest(_) => phase_only(BuildFailurePhase::Root),
            Self::TrustedSysroot { .. } | Self::TrustedCoreManifest(_) => classified(
                BuildFailurePhase::Root,
                BuildDiagnosticCode::CORE_SLOT_CORRUPT,
            ),
            Self::Resource(_) => classified(
                BuildFailurePhase::Root,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
        }
    }
}

impl ClassifyBuildFailure for BuildGraphDiscoveryError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::Locator(source) => source.classification(),
            Self::Resource(_) => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::Identity(_)
            | Self::IdentityCoordinateConflict { .. }
            | Self::ConflictingDependencyEdge { .. }
            | Self::InternalSourceClaim(_)
            | Self::InternalPrebuiltClaim(_) => phase_only(BuildFailurePhase::GraphIdentity),
            Self::MissingLocatorProjection(_) | Self::MissingDependencySpan(_) => {
                phase_only(BuildFailurePhase::Locator)
            }
            Self::ConflictingSourceLocator { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_CONFLICTING_SOURCE,
            ),
            Self::AmbiguousArtifact { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_AMBIGUOUS_ARTIFACT,
            ),
            Self::ConflictingNodeRepresentation { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_CONFLICTING_REPRESENTATION,
            ),
            Self::ReservedNodeClaim(_) => classified(
                BuildFailurePhase::GraphIdentity,
                BuildDiagnosticCode::GRAPH_RESERVED_IDENTITY,
            ),
        }
    }
}

impl ClassifyBuildFailure for DependencyLocatorError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::ArtifactNotFound { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_NOT_FOUND,
            ),
            Self::ArtifactNotRegularFile(_) | Self::InvalidArtifactSourceForm { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_WRONG_TYPE,
            ),
            Self::CoordinateMismatch { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_COORDINATE_MISMATCH,
            ),
            Self::AmbiguousArtifact { .. } => classified(
                BuildFailurePhase::Locator,
                BuildDiagnosticCode::LOCATOR_AMBIGUOUS_ARTIFACT,
            ),
            Self::ReservedArtifact { .. } => classified(
                BuildFailurePhase::GraphIdentity,
                BuildDiagnosticCode::GRAPH_RESERVED_IDENTITY,
            ),
            Self::ExecutableDependency { .. } => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_EXECUTABLE_DEPENDENCY,
            ),
            Self::ArtifactChangedDuringRead(_) => classified(
                BuildFailurePhase::Summary,
                BuildDiagnosticCode::PREBUILT_CHANGED,
            ),
            Self::ArtifactTooLarge { .. } | Self::Allocation(_) | Self::Resource(_) => classified(
                BuildFailurePhase::Summary,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::Summary { .. } => classified(
                BuildFailurePhase::Summary,
                BuildDiagnosticCode::PREBUILT_VIEW_INVALID,
            ),
            Self::UndeclaredDependency(_)
            | Self::MissingLocatorProjection(_)
            | Self::Manifest(_)
            | Self::Io { .. }
            | Self::SelfSourceLocator { .. } => phase_only(BuildFailurePhase::Locator),
        }
    }
}

impl ClassifyBuildFailure for ResolveBuildGraphError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::Resource(_) | Self::Allocation { .. } => classified(
                BuildFailurePhase::GraphCycleOrder,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::InvalidReservedNode { .. } => classified(
                BuildFailurePhase::GraphIdentity,
                BuildDiagnosticCode::GRAPH_RESERVED_IDENTITY,
            ),
            Self::ExecutableDependency { .. } => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_EXECUTABLE_DEPENDENCY,
            ),
            Self::MultipleVersions(_) => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_MULTIPLE_VERSIONS,
            ),
            Self::MissingTrustedCore
            | Self::MissingDirectCore { .. }
            | Self::TrustedCoreHasDependencies => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_MISSING_CORE,
            ),
            Self::InvalidSingleFileGraph => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_SINGLE_FILE_DEPENDENCY,
            ),
            Self::UnreachableNodes(_) => classified(
                BuildFailurePhase::GraphCycleOrder,
                BuildDiagnosticCode::GRAPH_UNREACHABLE_NODE,
            ),
            Self::Cycles(_) => classified(
                BuildFailurePhase::GraphCycleOrder,
                BuildDiagnosticCode::GRAPH_CYCLE,
            ),
            Self::Identity(_)
            | Self::MissingRoot(_)
            | Self::NodeIdentityMismatch { .. }
            | Self::InvalidRootRepresentation { .. }
            | Self::EdgeKeyMismatch(_)
            | Self::MissingEdgeEndpoint { .. }
            | Self::EdgeCoordinateMismatch { .. }
            | Self::DependencyRecordMismatch { .. }
            | Self::InvalidEdgeOrigin { .. } => phase_only(BuildFailurePhase::GraphIdentity),
            Self::InternalCyclePath(_)
            | Self::InternalCycleEdge { .. }
            | Self::InternalTopologicalState(_)
            | Self::InternalTopologicalLength { .. } => {
                phase_only(BuildFailurePhase::GraphCycleOrder)
            }
        }
    }
}

impl ClassifyBuildFailure for PrepareBuildGraphError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::PairedCompiler(_) => classified(
                BuildFailurePhase::Toolchain,
                BuildDiagnosticCode::CHILD_TOOL_MISMATCH,
            ),
            Self::CoreSourceKey(_)
            | Self::CoreLockIo { .. }
            | Self::InvalidCoreLockFileType(_)
            | Self::CoreLockPathChanged(_) => classified(
                BuildFailurePhase::CoreSlot,
                BuildDiagnosticCode::CORE_SLOT_CORRUPT,
            ),
            Self::MissingTrustedCore
            | Self::InvalidTrustedCoreRepresentation
            | Self::DuplicateTrustedCore => classified(
                BuildFailurePhase::GraphVersionKind,
                BuildDiagnosticCode::GRAPH_MISSING_CORE,
            ),
            Self::ArtifactSummaryChanged(_) | Self::PrebuiltProjectionChanged(_) => classified(
                BuildFailurePhase::PrebuiltArtifact,
                BuildDiagnosticCode::PREBUILT_CHANGED,
            ),
            Self::ArtifactSummary { .. } => classified(
                BuildFailurePhase::PrebuiltArtifact,
                BuildDiagnosticCode::PREBUILT_VIEW_INVALID,
            ),
            Self::ArtifactSnapshot { .. } | Self::ArtifactLengthOverflow(_) => {
                phase_only(BuildFailurePhase::PrebuiltArtifact)
            }
            Self::Resource(_) => classified(
                BuildFailurePhase::SourceSnapshot,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::Staging(_)
            | Self::ManifestSnapshot(_)
            | Self::ManifestChanged(_)
            | Self::SourceDiscovery(_)
            | Self::SingleFile(_)
            | Self::SourceLengthOverflow => phase_only(BuildFailurePhase::SourceSnapshot),
        }
    }
}

impl ClassifyBuildFailure for BuildGraphExecutionError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::MissingPreparedNode(_) | Self::RequestIdOverflow => {
                phase_only(BuildFailurePhase::ChildTransport)
            }
            Self::CacheKey(_, _) => phase_only(BuildFailurePhase::Cache),
            Self::CoreCompletion(_) => classified(
                BuildFailurePhase::CoreSlot,
                BuildDiagnosticCode::CORE_SLOT_CORRUPT,
            ),
            Self::CoreBootstrap(source) => source.classification(),
            Self::Prebuilt(_, source) => source.classification(),
            Self::Ordinary(_, source) => source.classification(),
        }
    }
}

impl BuildGraphExecutionError {
    /// Returns child-provided diagnostics without replacing their original codes.
    pub fn child_diagnostics(&self) -> Option<&[StructuredDiagnosticV1]> {
        match self {
            Self::CoreBootstrap(source) => match source.as_ref() {
                CoreBootstrapExecutionError::ChildFailure(diagnostics) => Some(diagnostics),
                _ => None,
            },
            Self::Ordinary(_, source) => match source.as_ref() {
                OrdinarySourceExecutionError::ChildFailure(diagnostics) => Some(diagnostics),
                _ => None,
            },
            _ => None,
        }
    }
}

impl ClassifyBuildFailure for CoreBootstrapExecutionError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::ChildProtocol(_) => classified(
                BuildFailurePhase::ChildTransport,
                BuildDiagnosticCode::CHILD_PROTOCOL,
            ),
            Self::ChildTransport(source) => source.classification(),
            Self::ChildFailure(_) => phase_only(BuildFailurePhase::ChildDiagnostic),
            Self::SourceSnapshot(source) => source.classification(),
            Self::SourceChanged { .. } => classified(
                BuildFailurePhase::CoreSlot,
                BuildDiagnosticCode::CORE_SOURCE_CHANGED,
            ),
            Self::OutputSnapshot(_) | Self::OutputLengthOverflow => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_OUTPUT_MISSING,
            ),
            Self::ChildResult(_) => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_RESPONSE_MISMATCH,
            ),
            Self::OutputSummary(_)
            | Self::Staging(_)
            | Self::ReceiptValidation(_)
            | Self::ReceiptHash(_)
            | Self::Completion(_) => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_OUTPUT_PLAN_MISMATCH,
            ),
            Self::PublishReceipt(_) => classified(
                BuildFailurePhase::CachePublish,
                BuildDiagnosticCode::CACHE_PUBLISH,
            ),
            Self::SlotAlreadyReusable
            | Self::RequestPlan(_)
            | Self::Resource(_)
            | Self::ReloadSourceManifest(_)
            | Self::SourceKey(_) => classified(
                BuildFailurePhase::CoreSlot,
                BuildDiagnosticCode::CORE_BOOTSTRAP_FAILED,
            ),
        }
    }
}

impl ClassifyBuildFailure for OrdinarySourceExecutionError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::CacheStore(source) => source.classification(),
            Self::CacheCompletion(CacheCompletionError::Resource(_)) => classified(
                BuildFailurePhase::Cache,
                BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
            ),
            Self::CacheCompletion(_) => classified(
                BuildFailurePhase::Cache,
                BuildDiagnosticCode::CACHE_ENTRY_CORRUPT,
            ),
            Self::ChildProtocol(_) => classified(
                BuildFailurePhase::ChildTransport,
                BuildDiagnosticCode::CHILD_PROTOCOL,
            ),
            Self::ChildTransport(source) => source.classification(),
            Self::ChildFailure(_) => phase_only(BuildFailurePhase::ChildDiagnostic),
            Self::OutputLayout(_) | Self::OutputSnapshot(_) => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_OUTPUT_MISSING,
            ),
            Self::ChildResult(_) => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_RESPONSE_MISMATCH,
            ),
            Self::Completion(_)
            | Self::ConeRecord(_)
            | Self::ReceiptValidation(_)
            | Self::ReceiptHash(_) => classified(
                BuildFailurePhase::ChildOutput,
                BuildDiagnosticCode::CHILD_OUTPUT_PLAN_MISMATCH,
            ),
            Self::NotOrdinarySource(_) | Self::RequestPlan(_) | Self::Resource(_) => {
                phase_only(BuildFailurePhase::ChildTransport)
            }
            Self::CacheKey(_) => phase_only(BuildFailurePhase::Cache),
        }
    }
}

impl ClassifyBuildFailure for PrebuiltCompletionError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::StalePrebuiltDependency { .. } => classified(
                BuildFailurePhase::PrebuiltArtifact,
                BuildDiagnosticCode::PREBUILT_STALE_DEPENDENCY,
            ),
            Self::CandidateDisagreement { .. } => classified(
                BuildFailurePhase::PrebuiltArtifact,
                BuildDiagnosticCode::PREBUILT_SUMMARY_MISMATCH,
            ),
            Self::NotPrebuilt(_)
            | Self::EmptyCandidateSet(_)
            | Self::DuplicateCompletedNode(_)
            | Self::CurrentNodeAlreadyCompleted(_)
            | Self::MissingTrustedCore
            | Self::TrustedCoreReopen(_)
            | Self::CandidateArtifact { .. }
            | Self::CandidatePlan { .. } => classified(
                BuildFailurePhase::PrebuiltArtifact,
                BuildDiagnosticCode::PREBUILT_VIEW_INVALID,
            ),
        }
    }
}

impl ClassifyBuildFailure for CompileCacheStoreError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::Io {
                operation: CacheIoOperation::Lock,
                ..
            }
            | Self::WrongLock => {
                classified(BuildFailurePhase::Cache, BuildDiagnosticCode::CACHE_LOCK)
            }
            Self::Io {
                operation: CacheIoOperation::Publish,
                ..
            }
            | Self::Io {
                operation:
                    CacheIoOperation::Write | CacheIoOperation::Sync | CacheIoOperation::SetPermissions,
                ..
            }
            | Self::CandidateVerificationMissing
            | Self::CandidateVerificationMismatch
            | Self::PublishedEntryDisappeared(_)
            | Self::AtomicNoReplaceUnavailable(_) => classified(
                BuildFailurePhase::CachePublish,
                BuildDiagnosticCode::CACHE_PUBLISH,
            ),
            Self::NondeterministicProduction(_) => classified(
                BuildFailurePhase::CachePublish,
                BuildDiagnosticCode::CACHE_NONDETERMINISTIC_PRODUCTION,
            ),
            Self::Io { .. } | Self::Snapshot { .. } | Self::ReceiptEncode(_) => {
                classified(BuildFailurePhase::Cache, BuildDiagnosticCode::CACHE_IO)
            }
            Self::InvalidNamespace(_)
            | Self::InvalidPathType { .. }
            | Self::UnexpectedEntryContents { .. }
            | Self::ReceiptDecode(_)
            | Self::ReceiptKeyMismatch { .. }
            | Self::ReceiptTooLarge { .. }
            | Self::LengthOverflow => classified(
                BuildFailurePhase::Cache,
                BuildDiagnosticCode::CACHE_ENTRY_CORRUPT,
            ),
        }
    }
}

impl ClassifyBuildFailure for ChildTransportError {
    fn classification(&self) -> BuildFailureClassification {
        match self {
            Self::CompilerChanged(_) => classified(
                BuildFailurePhase::Toolchain,
                BuildDiagnosticCode::CHILD_TOOL_MISMATCH,
            ),
            Self::Request(_)
            | Self::Response { .. }
            | Self::RequestIdMismatch { .. }
            | Self::UnexpectedStderr(_) => classified(
                BuildFailurePhase::ChildTransport,
                BuildDiagnosticCode::CHILD_PROTOCOL,
            ),
            Self::Signal | Self::ExitMismatch { .. } => classified(
                BuildFailurePhase::ChildTransport,
                BuildDiagnosticCode::CHILD_EXIT,
            ),
            Self::MissingExecutableParent { .. }
            | Self::Spawn { .. }
            | Self::MissingPipe(_)
            | Self::WriteRequest(_)
            | Self::Wait(_)
            | Self::ReadStream { .. }
            | Self::StreamLengthOverflow(_)
            | Self::StreamTooLarge { .. }
            | Self::ReaderPanicked(_) => classified(
                BuildFailurePhase::ChildTransport,
                BuildDiagnosticCode::CHILD_TRANSPORT,
            ),
        }
    }
}

#[cfg(test)]
mod tests;
