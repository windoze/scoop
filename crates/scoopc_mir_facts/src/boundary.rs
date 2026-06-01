//! MIR-published source contracts for later effect boundary materialization.

use scoopc_ids::{BodyBlockId, SiteId, StageArtifactKey};
use scoopc_types::TypeId;

use crate::common::{FactIdentity, MirBodyReference};
use crate::effects::MirSiteKind;

/// Boundary source contracts discoverable from MIR site anchors.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MirBoundaryFacts {
    pub source_contracts: Vec<BoundarySourceContract>,
}

impl MirBoundaryFacts {
    /// Return whether no boundary source contracts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.source_contracts.is_empty()
    }
}

/// Structure needed by later stages to materialize a site boundary without re-scanning MIR slices.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BoundarySourceContract {
    pub identity: FactIdentity,
    pub instance: StageArtifactKey,
    pub body: MirBodyReference,
    pub site_id: SiteId,
    pub kind: MirSiteKind,
    pub anchor: BoundaryAnchor,
    pub result_local: Option<u32>,
    pub carrier: Option<BoundaryOperandSource>,
    pub args: Vec<BoundaryOperandSource>,
    pub closure_env: Option<ClosureEnvDecomposition>,
}

/// Statement or terminator anchor for a boundary-producing MIR site.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BoundaryAnchor {
    Statement {
        block: BodyBlockId,
        statement_index: u32,
    },
    Terminator {
        block: BodyBlockId,
    },
}

/// Stable operand source used by boundary materialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BoundaryOperandSource {
    Local { local: u32, ty: Option<TypeId> },
    Const { kind: String },
}

/// Closure environment payload sources when a callable carrier is a known closure.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ClosureEnvDecomposition {
    pub fn_ptr: String,
    pub env: BoundaryOperandSource,
}
