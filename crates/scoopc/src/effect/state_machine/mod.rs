//! Shared ordinary-callee suspend planning helpers.
//!
//! The previous handle state-machine lowering has been removed; this
//! module now retains only the analysis pieces still consumed by the current
//! LLVM backend for ordinary callee suspend/resume planning.

mod analysis;

#[cfg(feature = "llvm")]
pub(crate) use analysis::{
    CalleeSuspendPlan, SuspendCallAnalysis, build_ordinary_callee_suspend_plan_with_context,
    collect_known_fun_call_suspendability, function_ty_declared_effectful,
    hir_ty_is_function_value,
};
