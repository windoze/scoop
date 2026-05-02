//! refactor 主线的 late-lowered internal representation 子系统。
//!
//! P5-T01 在这里先固定独立边界。当前仓库中的实际模块路径与 TODO 推荐拆分的映射如下：
//! - `ir.rs` 对应 late-lowered IR 容器；
//! - `builder.rs` 负责编排 canonical MIR snapshot + `MaterializedEffectFacts` 的整体组装；
//! - `segment.rs` 承接 TODO 推荐的 whole-function segmentation / boundary 选择骨架；
//! - `frame.rs` 承接 TODO 推荐的 frame lifting 与显式控制流合同补全；
//! - `materialize.rs` 承接 TODO 推荐的 `materialize.rs` 职责：物化 `Step_F`、dynamic
//!   `invoke`、continuation object、resume interfaces 与 boundary lowering contract；
//! - `opt.rs` 承接 TODO 推荐的 late-lowered 窄后处理：在不改变 canonical contract 的前提下，
//!   做闭世界 devirtualization / inlining / DCE；
//! - `dump.rs` 提供稳定 formatter；

pub(crate) mod builder;
pub mod dump;
pub(crate) mod frame;
pub mod ir;
pub(crate) mod materialize;
pub(crate) mod opt;
pub(crate) mod segment;

pub(crate) use builder::LateLoweredProgramBuilder;
pub use dump::render_late_lowered_program;
pub use ir::{LateLoweredCallable, LateLoweredProgram};
pub(crate) use opt::optimize_program;

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

    #[error(
        "refactor late-lowering stage 在 StepSchema s{step_schema} 上找不到 effect family `{effect_fqn}` 对应的 resume interface"
    )]
    MissingResumeInterfaceFamily {
        step_schema: u32,
        effect_fqn: String,
    },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 site{site_id} 上找不到 `{kind}` boundary 对应的结果 local"
    )]
    MissingBoundaryResultLocal {
        root_fqn: String,
        site_id: u32,
        kind: &'static str,
    },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 resume site{site_id} 上找不到配对的 runtime-error boundary"
    )]
    MissingPairedRuntimeErrorBoundary { root_fqn: String, site_id: u32 },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 runtime-error site{site_id} 上找不到配对的 resume boundary"
    )]
    MissingPairedResumeBoundary { root_fqn: String, site_id: u32 },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 StepSchema s{step_schema} 上找不到 case c{case_tag}"
    )]
    MissingInputStepCase {
        root_fqn: String,
        step_schema: u32,
        case_tag: u32,
    },

    #[error(
        "refactor late-lowering stage 无法把 `{concrete_op}` 从 input StepSchema s{input_step_schema} 投影到 output StepSchema s{output_step_schema}` 上"
    )]
    MissingProjectedStepCase {
        root_fqn: String,
        input_step_schema: u32,
        output_step_schema: u32,
        concrete_op: String,
    },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 resume site{site_id} 上找不到 MIR resume metadata"
    )]
    MissingResumeSiteMetadata { root_fqn: String, site_id: u32 },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 resume site{site_id} 上找不到 runtime-error effect 身份"
    )]
    MissingResumeRuntimeErrorEffect { root_fqn: String, site_id: u32 },

    #[error(
        "refactor late-lowering stage 无法把 `{root_fqn}` 的 site{site_id} 上的 t{ty} 解释成稳定 effect family"
    )]
    UnsupportedEffectFamilyType {
        root_fqn: String,
        site_id: u32,
        ty: u32,
    },

    #[error(
        "refactor late-lowering stage 在 `{root_fqn}` 的 resume site{site_id} 的 out StepSchema s{step_schema} 上找不到 runtime-error case"
    )]
    MissingRuntimeErrorCaseInResumeStep {
        root_fqn: String,
        site_id: u32,
        step_schema: u32,
    },

    #[error("refactor late-lowering stage 在 `{root_fqn}` 上找不到已 intern 的 builtin 类型集合")]
    MissingBuiltinTypes { root_fqn: String },
}

impl From<crate::effect_facts::EffectFactsError> for EffectLoweringError {
    fn from(error: crate::effect_facts::EffectFactsError) -> Self {
        Self::EffectFacts(Box::new(error))
    }
}
