//! Scoped HIR-to-MIR semantic closure validation for the M23-6 profile.
//!
//! A checked MIR section recursively borrows terminal dependency sections.
//! The prepared artifacts and both proof layers therefore remain inside
//! callback-scoped typed arenas and cannot be detached from the graph that
//! established them.

mod errors;
mod model;
mod validation;

pub use errors::*;
pub use model::*;
