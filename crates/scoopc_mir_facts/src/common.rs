//! Shared identity records for MIR fact groups.

use scoopc_ids::{BodyVersionKey, CanonicalTextKey, StageArtifactKey};
use scoopc_project_model::StableConeKey;
use scoopc_source::SourceMapSpan;
use scoopc_types::TypeId;

/// Stage-neutral identity for a MIR-published fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FactIdentity {
    pub key: CanonicalTextKey,
    pub display_name: String,
    pub cone: StableConeKey,
    pub source: Option<SourceMapSpan>,
}

impl FactIdentity {
    /// Create an identity from a canonical key, readable name, and owning cone.
    pub fn new(
        key: CanonicalTextKey,
        display_name: impl Into<String>,
        cone: StableConeKey,
        source: Option<SourceMapSpan>,
    ) -> Self {
        Self {
            key,
            display_name: display_name.into(),
            cone,
            source,
        }
    }

    /// Return the canonical text used for stable maps and duplicate checks.
    pub fn canonical_text(&self) -> &str {
        self.key.as_str()
    }
}

/// Stable reference to a direct-style or materialized MIR body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MirBodyReference {
    pub key: BodyVersionKey,
    pub owner: CanonicalTextKey,
    pub fqn: String,
    pub ty: Option<TypeId>,
}

impl MirBodyReference {
    /// Create a body reference without depending on MIR `Body` internals.
    pub fn new(
        key: BodyVersionKey,
        owner: CanonicalTextKey,
        fqn: impl Into<String>,
        ty: Option<TypeId>,
    ) -> Self {
        Self {
            key,
            owner,
            fqn: fqn.into(),
            ty,
        }
    }
}

/// Stable reference to a MIR-stage artifact such as a snapshot or revision.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MirArtifactReference {
    pub key: StageArtifactKey,
    pub label: String,
}

impl MirArtifactReference {
    /// Create an artifact reference from a stage-independent artifact key.
    pub fn new(key: StageArtifactKey, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }
}
