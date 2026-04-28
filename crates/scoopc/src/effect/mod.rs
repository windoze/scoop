//! Shared effect analysis and state-machine middle-end support.
//!
//! LLVM codegen consumes the state-machine contracts from this module, but the
//! planning and summary logic lives here so it can depend on MIR/shared facts
//! instead of backend lowering context.

pub(crate) mod analysis;
pub(crate) mod state_machine;

#[cfg(not(feature = "llvm"))]
pub(crate) mod step_summary;
