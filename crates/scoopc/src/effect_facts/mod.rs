//! refactor 主线的 effect-facts 子系统。
//!
//! P4 起，authoritative effect contract 必须集中在这套 side table 中，而不是继续散落在
//! legacy `effect/analysis`、`ProgramFacts` 或 `mir::summary::InstanceSummary` 里。

pub mod builder;
pub mod dump;
pub mod facts;
pub mod schema;
pub mod solver;

pub use builder::MaterializedEffectFactsBuilder;
pub use dump::render_materialized_effect_facts;
pub use facts::{
    BodyEffectFacts, CallableEffectFacts, CanonicalMirQuerySurface, MaterializedEffectFacts,
    MirSnapshotBinding,
};
pub use schema::{ContinuationSchema, ContinuationSchemaId, StepSchema, StepSchemaId};
pub use solver::MaterializedEffectFactsSolver;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EffectFactsError {
    #[error(transparent)]
    Mir(#[from] crate::mir::MirLowerError),

    #[error(transparent)]
    Materialize(#[from] Box<crate::mir::MirMaterializeError>),

    #[error("refactor effect-facts stage requires a canonical materialized MIR snapshot from P3")]
    MissingMaterializedMirSnapshot,
}
