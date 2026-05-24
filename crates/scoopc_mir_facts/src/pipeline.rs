//! Metadata describing the MIR pass pipeline schedule and results.

use scoopc_ids::StageArtifactKey;

/// Published metadata for the MIR pass pipeline execution.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirPassPipelineMetadata {
    pub runs: Vec<MirPassRun>,
}

impl MirPassPipelineMetadata {
    /// Return whether no pass run has been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// One scheduled MIR pass execution and its artifact revisions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirPassRun {
    pub pass: MirPassKind,
    pub enabled: bool,
    pub input_revision: Option<StageArtifactKey>,
    pub output_revision: Option<StageArtifactKey>,
    pub changed_bodies: usize,
    pub changed_summaries: usize,
    pub produced_escape_facts: bool,
}

impl MirPassRun {
    /// Record a pass execution without depending on pass implementation types.
    pub fn new(pass: MirPassKind, enabled: bool) -> Self {
        Self {
            pass,
            enabled,
            input_revision: None,
            output_revision: None,
            changed_bodies: 0,
            changed_summaries: 0,
            produced_escape_facts: false,
        }
    }
}

/// Stable names for the MIR pass family tracked by P3.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MirPassKind {
    Devirtualization,
    SummaryDrivenInlining,
    EscapeAnalysis,
    ClosureSimplification,
    Cleanup,
    SummaryRefresh,
    Other(String),
}

impl MirPassKind {
    /// Return a stable dump label for this pass kind.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Devirtualization => "devirtualization",
            Self::SummaryDrivenInlining => "summary-driven-inlining",
            Self::EscapeAnalysis => "escape-analysis",
            Self::ClosureSimplification => "closure-simplification",
            Self::Cleanup => "cleanup",
            Self::SummaryRefresh => "summary-refresh",
            Self::Other(name) => name.as_str(),
        }
    }
}
