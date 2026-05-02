//! refactor 主线的 late-lowered internal representation 子系统。
//!
//! P5-T01 在这里先固定独立边界。当前仓库中的实际模块路径与 TODO 推荐拆分的映射如下：
//! - `ir.rs` 对应 late-lowered IR 容器；
//! - `builder.rs` 负责把 canonical MIR snapshot + `MaterializedEffectFacts` 组装成初始
//!   `LateLoweredProgram`，当前承接了 TODO 推荐 `materialize.rs` 的最小职责；
//! - `dump.rs` 提供稳定 formatter；
//! - TODO 推荐的 `segment.rs` / `frame.rs` / `opt.rs` 会在后续 P5 任务里补入；本任务先不提前伪造。

pub(crate) mod builder;
pub mod dump;
pub mod ir;

pub(crate) use builder::LateLoweredProgramBuilder;
pub use dump::render_late_lowered_program;
pub use ir::{LateLoweredCallable, LateLoweredProgram};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EffectLoweringError {
    #[error(transparent)]
    EffectFacts(#[from] Box<crate::effect_facts::EffectFactsError>),

    #[error("refactor late-lowering stage 找不到 `{root_fqn}` 对应的 callable facts")]
    MissingCallableFacts { root_fqn: String },

    #[error("refactor late-lowering stage 找不到 `{root_fqn}` 对应的 StepSchema s{step_schema}")]
    MissingStepSchema { root_fqn: String, step_schema: u32 },

    #[error(
        "refactor late-lowering stage 看到的 StepSchema s{step_schema} 在 case c{case_tag} 上缺少 continuation schema k{continuation_schema}"
    )]
    MissingContinuationSchema {
        step_schema: u32,
        continuation_schema: u32,
        case_tag: u32,
    },

    #[error(
        "refactor late-lowering stage 看到的 `{root_fqn}` invoke args tuple(t{callable_args_tuple}) 与 StepSchema s{step_schema} 的 invoke args tuple(t{step_args_tuple}) 不一致"
    )]
    InvokeArgsTupleMismatch {
        root_fqn: String,
        step_schema: u32,
        callable_args_tuple: u32,
        step_args_tuple: u32,
    },

    #[error(
        "refactor late-lowering stage 看到的 canonical snapshot instance 数量({snapshot_instances}) 与 callable facts 数量({callable_facts}) 不一致"
    )]
    SnapshotCallableCountMismatch {
        snapshot_instances: usize,
        callable_facts: usize,
    },
}

impl From<crate::effect_facts::EffectFactsError> for EffectLoweringError {
    fn from(error: crate::effect_facts::EffectFactsError) -> Self {
        Self::EffectFacts(Box::new(error))
    }
}
