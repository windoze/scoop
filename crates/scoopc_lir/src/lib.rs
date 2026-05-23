//! LIR stage implementation for the Scoop compiler.
//!
//! This crate owns late effect/control lowering, the LIR body model, and LIR
//! optimization passes. It consumes MIR, MIR facts, published effect facts, and
//! LIR facts without depending on the `scoopc` facade or frontend stages.

#![forbid(unsafe_code)]

pub mod effect_facts;
pub mod effect_lowered;

pub use effect_lowered::*;
pub use scoopc_mir::mir;
pub use scoopc_mir::stable_id;

pub mod opt {
    pub use scoopc_project_model::{InvalidOptLevel, OptLevel};
}
pub mod source {
    pub use scoopc_source::*;
}
pub mod span {
    pub use scoopc_span::*;
}
pub mod ty {
    pub use scoopc_types::*;
}

#[cfg(test)]
pub use scoopc_mir::{parser, resolve, session, typecheck};
