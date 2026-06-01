//! MIR-published callable value and result provenance facts.

use scoopc_ids::{BodyBlockId, CanonicalTextKey, SiteId, StageArtifactKey};

use crate::common::{FactIdentity, MirBodyReference};

/// Provenance facts published from materialized MIR and pass summaries.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirProvenanceFacts {
    pub callable_values: Vec<CallableValueProvenanceFact>,
    pub results: Vec<ResultProvenanceFact>,
}

impl MirProvenanceFacts {
    /// Return whether no provenance facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.callable_values.is_empty() && self.results.is_empty()
    }
}

/// Stable provenance for a callable value stored in a local.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallableValueProvenanceFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub local: u32,
    pub block: Option<BodyBlockId>,
    pub site_id: Option<SiteId>,
    pub provenance: CallableValueProvenance,
}

/// Callable-value points-to family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CallableValueProvenance {
    DirectFunction { fqn: String },
    KnownInstance { key: CanonicalTextKey },
    KnownClosure { fn_ptr: String },
    Param { index: usize },
    Unknown,
}

/// Stable result provenance for one materialized instance summary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResultProvenanceFact {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub callable: CanonicalTextKey,
    pub provenance: ResultProvenance,
    pub summary_overridden: bool,
}

/// Stable result provenance family mirrored from MIR pass summaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ResultProvenance {
    Unit,
    Param {
        index: usize,
    },
    DirectFunction {
        fqn: String,
    },
    KnownClosure {
        fn_ptr: String,
    },
    TopLevelValue {
        fqn: String,
    },
    PerformResult {
        op_fqn: String,
    },
    Join {
        sources: Vec<ResultProvenanceSource>,
    },
    Unknown,
}

/// One source inside joined result provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ResultProvenanceSource {
    Param { index: usize },
    DirectFunction { fqn: String },
    KnownClosure { fn_ptr: String },
    TopLevelValue { fqn: String },
    PerformResult { op_fqn: String },
}
