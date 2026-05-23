//! LIR-owned ordinary-callee suspend planning contract.
//!
//! LLVM codegen consumes this module through the LIR owner instead of reaching
//! back into the effect-analysis stage. The planner still operates on the
//! transitional LIR source payloads until source bodies are fully LIR-native.

mod analysis;
mod plan;

pub(crate) use analysis::{
    ContinuationEscapeFacts, EffectAnalysisCtx, EffectAnalysisFacts, EffectConstructorCall,
    EffectContinuationResume, EffectFieldFact, EffectFieldOwnerKind, EffectGlobalRootKind,
    KnownLocalMetadata,
};
pub(crate) use plan::{
    CalleeSuspendPlan, SuspendCallAnalysis, build_ordinary_callee_suspend_plan_with_context,
    function_ty_declared_effectful, hir_ty_is_function_value,
};
