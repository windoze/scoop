//! References to the HIR type universe and source-cone ownership context.

use std::path::PathBuf;

use scoop_project_model::{ConeId, StableConeKey};
use scoopc_ids::CanonicalTextKey;
use scoopc_source::{SourceId, SourceMapSpan};
use scoopc_types::BuiltinTypes;

/// Facts that reference, but do not duplicate, the HIR type context.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypeContextFacts {
    pub type_universe: Option<TypeContextReference>,
    pub stable_type_params: Vec<StableTypeParamFact>,
    pub source_cones: Vec<SourceConeFact>,
}

impl TypeContextFacts {
    /// Return whether no type-context references have been published yet.
    pub fn is_empty(&self) -> bool {
        self.type_universe.is_none()
            && self.stable_type_params.is_empty()
            && self.source_cones.is_empty()
    }
}

/// Stable reference to the HIR-owned `TypeStore` used by these facts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TypeContextReference {
    pub label: String,
    pub type_count: usize,
    pub builtins: Option<BuiltinTypes>,
}

/// Stable owner/index key for a type or effect parameter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StableTypeParamFact {
    pub owner: CanonicalTextKey,
    pub index: u32,
    pub key: CanonicalTextKey,
    pub name: String,
    pub source: Option<SourceMapSpan>,
}

/// Source file ownership by cone after source-cone graph resolution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceConeFact {
    pub source_id: Option<SourceId>,
    pub source_path: PathBuf,
    pub cone_id: ConeId,
    pub stable_key: StableConeKey,
}
