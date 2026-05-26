//! Common identity records shared by HIR fact groups.

use scoop_project_model::StableConeKey;
use scoopc_ids::CanonicalTextKey;
use scoopc_source::SourceMapSpan;

/// Stage-neutral identity for a declaration, root, or native binding fact.
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
