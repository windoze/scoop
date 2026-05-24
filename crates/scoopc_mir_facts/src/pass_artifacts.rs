//! Metadata for MIR pass artifacts and revisions.

use scoopc_ids::StageArtifactKey;

use crate::common::MirBodyReference;

/// Metadata published for pass-visible MIR artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PassArtifactMetadata {
    pub revisions: Vec<PassArtifactRevision>,
    pub callable_body_overrides: Vec<CallableBodyArtifact>,
    pub summary_revisions: Vec<SummaryArtifact>,
    pub escape_facts: Vec<EscapeFactsArtifact>,
}

impl PassArtifactMetadata {
    /// Return whether no pass artifact metadata has been published yet.
    pub fn is_empty(&self) -> bool {
        self.revisions.is_empty()
            && self.callable_body_overrides.is_empty()
            && self.summary_revisions.is_empty()
            && self.escape_facts.is_empty()
    }
}

/// Revision marker for a pass artifact table.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PassArtifactRevision {
    pub key: StageArtifactKey,
    pub pass_name: String,
    pub revision: u32,
}

impl PassArtifactRevision {
    /// Create a revision marker for stable dumps and verifier checks.
    pub fn new(key: StageArtifactKey, pass_name: impl Into<String>, revision: u32) -> Self {
        Self {
            key,
            pass_name: pass_name.into(),
            revision,
        }
    }
}

/// Metadata for a pass-published callable body override.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallableBodyArtifact {
    pub revision: StageArtifactKey,
    pub body: MirBodyReference,
}

impl CallableBodyArtifact {
    /// Create metadata for a callable body override.
    pub fn new(revision: StageArtifactKey, body: MirBodyReference) -> Self {
        Self { revision, body }
    }
}

/// Metadata for a pass-published summary table entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SummaryArtifact {
    pub revision: StageArtifactKey,
    pub owner: StageArtifactKey,
}

impl SummaryArtifact {
    /// Create metadata for a summary artifact scoped to an instance or family.
    pub fn new(revision: StageArtifactKey, owner: StageArtifactKey) -> Self {
        Self { revision, owner }
    }
}

/// Metadata for MIR escape facts produced by a pass revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EscapeFactsArtifact {
    pub revision: StageArtifactKey,
    pub body_count: usize,
}

impl EscapeFactsArtifact {
    /// Create metadata for a published escape-facts table.
    pub fn new(revision: StageArtifactKey, body_count: usize) -> Self {
        Self {
            revision,
            body_count,
        }
    }
}
