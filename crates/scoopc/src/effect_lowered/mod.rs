//! refactor 主线的 late-lowered internal representation 子系统。
//!
//! P5-T01 在这里先固定独立边界。当前仓库中的实际模块路径与 TODO 推荐拆分的映射如下：
//! - `ir.rs` 对应 late-lowered IR 容器；
//! - `builder.rs` 负责把 canonical MIR snapshot + `MaterializedEffectFacts` 组装成初始
//!   `LateLoweredProgram`，当前承接了 TODO 推荐 `materialize.rs` 的最小职责；
//! - `segment.rs` 承接 TODO 推荐的 whole-function segmentation / boundary 选择骨架；
//! - `frame.rs` 承接 TODO 推荐的 frame lifting 与显式控制流合同补全；
//! - `dump.rs` 提供稳定 formatter；
//! - TODO 推荐的 `opt.rs` 会在后续 P5 任务里补入；当前不提前伪造空壳。

pub(crate) mod builder;
pub mod dump;
pub(crate) mod frame;
pub mod ir;
pub(crate) mod segment;

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

    #[error("refactor late-lowering stage 找不到 `{root_fqn}` 对应的 body facts")]
    MissingBodyFacts { root_fqn: String },

    #[error("refactor late-lowering stage 找不到 `{root_fqn}` 对应的 StepSchema s{step_schema}")]
    MissingStepSchema { root_fqn: String, step_schema: u32 },

    #[error("refactor late-lowering stage 在 `{root_fqn}` 的 site{site_id} 上找不到 P4 site facts")]
    MissingSiteFacts { root_fqn: String, site_id: u32 },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 site{site_id} 上看到的 P4 site facts 种类不是期望的 `{expected}`，而是 `{actual}`"
    )]
    UnexpectedSiteFactsKind {
        root_fqn: String,
        site_id: u32,
        expected: &'static str,
        actual: &'static str,
    },

    #[error(
        "refactor late-lowering stage 看到的 StepSchema s{step_schema} 在 case c{case_tag} 上缺少 continuation schema k{continuation_schema}"
    )]
    MissingContinuationSchema {
        step_schema: u32,
        continuation_schema: u32,
        case_tag: u32,
    },

    #[error(
        "refactor late-lowering stage 看到的 StepSchema s{step_schema} 在 case c{case_tag} 上引用的 continuation schema k{continuation_schema} 声明 out_step_schema=s{out_step_schema}，与当前 return-step contract 不一致"
    )]
    ContinuationOutStepSchemaMismatch {
        step_schema: u32,
        continuation_schema: u32,
        case_tag: u32,
        out_step_schema: u32,
    },

    #[error(
        "refactor late-lowering stage 看到的 StepSchema s{step_schema} 在 case c{case_tag} 上引用的 continuation schema k{continuation_schema} 声明 answer_ty=t{answer_ty}，但当前 return-step complete_ty=t{complete_ty}"
    )]
    ContinuationAnswerTyMismatch {
        step_schema: u32,
        continuation_schema: u32,
        case_tag: u32,
        answer_ty: u32,
        complete_ty: u32,
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

    #[error(
        "refactor late-lowering stage 无法为 `{root_fqn}` 的 boundary `{description}` 绑定 owner/resume state"
    )]
    UnboundBoundary {
        root_fqn: String,
        description: String,
    },

    #[error("refactor late-lowering stage 在 `{root_fqn}` 上找不到已 intern 的 builtin 类型集合")]
    MissingBuiltinTypes { root_fqn: String },
}

impl From<crate::effect_facts::EffectFactsError> for EffectLoweringError {
    fn from(error: crate::effect_facts::EffectFactsError) -> Self {
        Self::EffectFacts(Box::new(error))
    }
}
