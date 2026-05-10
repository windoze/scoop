//! Shared ordinary-callee suspend planning helpers.
//!
//! The previous handle state-machine lowering has been removed; this
//! module now retains only the analysis pieces still consumed by the current
//! LLVM backend for ordinary callee suspend/resume planning.

mod analysis;

pub(crate) use analysis::CalleeSuspendPlan;
