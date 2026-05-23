//! effect-facts 子系统。
//!
//! P4 起，authoritative effect contract 必须集中在这套 side table 中，而不是继续散落在
//! legacy `effect/analysis`、HIR source-site contracts 或 `mir::summary::InstanceSummary` 里。

pub mod builder;
pub mod dump;
pub mod facts;
mod product;
pub mod schema;
pub mod solver;

pub use builder::MaterializedEffectFactsBuilder;
pub use dump::render_materialized_effect_facts;
pub use facts::{
    BlockEffectFacts, BodyEffectFacts, CallSiteEffectFacts, CallSiteKind, CallSiteTarget,
    CallTargetMode, CallableAbiKind, CallableEffectFacts, CanonicalMirQuerySurface,
    ClassCtorSiteEffectFacts, EffectOwnedTypeContext, EffectPrecision, HandleArmEffectFacts,
    HandleSiteEffectFacts, MaterializedEffectFacts, MirSnapshotBinding, NestedHandleClassification,
    PerformSiteEffectFacts, ResumeSiteEffectFacts, SiteEffectFacts,
};
pub(crate) use facts::{BodyEffectSolverFacts, HandleSiteSolverFacts};
pub use product::EffectFactsProductError;
pub use schema::{
    CaseSet, CaseTag, ConcreteOpKey, ContinuationSchema, ContinuationSchemaId, EffectFamilyKey,
    ImplPlan, StepCaseFact, StepSchema, StepSchemaId,
};
pub use solver::MaterializedEffectFactsSolver;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EffectFactsError {
    #[error(transparent)]
    Mir(#[from] crate::mir::MirLowerError),

    #[error(transparent)]
    Materialize(#[from] Box<crate::mir::MirMaterializeError>),

    #[error(transparent)]
    Session(#[from] crate::session::SessionError),

    #[error(transparent)]
    Parse(#[from] crate::parser::ParseError),

    #[error(transparent)]
    TypeEnv(#[from] Box<crate::typecheck::TypeEnvError>),

    #[error(transparent)]
    TypeLower(#[from] Box<crate::typecheck::TypeLowerError>),

    #[error(transparent)]
    VtableLayout(#[from] crate::vtable::VtableLayoutError),

    #[error(transparent)]
    ItableLayout(#[from] crate::itable::ItableLayoutError),

    #[error(transparent)]
    Product(#[from] EffectFactsProductError),

    #[error("effect-facts stage frontend setup failed: {message}")]
    Frontend { message: String },

    #[error("effect-facts stage 找不到 callable root `{fqn}` 的 MIR 声明头")]
    MissingCallableRoot { fqn: String },

    #[error("effect row 中出现了无法作为 canonical effect identity 的项：{ty}")]
    UnsupportedEffectTerm { ty: String },

    #[error("call site 无法为 `{callable}` 构建 surface contract")]
    MissingCallableSurfaceContract { callable: String },

    #[error("callable `{callable}` 的 step schema 中缺少 effect op `{op_fqn}` 对应的 case")]
    MissingCallableCase { callable: String, op_fqn: String },

    #[error("找不到 effect type `{effect_fqn}` 的声明头")]
    MissingEffectTypeSymbol { effect_fqn: String },

    #[error("effect `{effect_fqn}` 的 type args 数量不匹配：期望 {expected} 个，实际 {found} 个")]
    EffectTypeArgArityMismatch {
        effect_fqn: String,
        expected: usize,
        found: usize,
    },

    #[error("找不到声明文件 `{path}` 的 type-lowering 上下文")]
    MissingDeclFileContext { path: String },

    #[error("effect op `{op_fqn}` 的声明头不完整：{detail}")]
    MalformedEffectOpSignature {
        op_fqn: String,
        detail: &'static str,
    },
}
