//! Scoped HIR semantic closure validation for the M23-6 layout profile.
//!
//! Checked type sections borrow both their resolved wire sections and checked
//! terminal dependency sections. They therefore live in a typed arena for one
//! validation scope and cannot escape into an owning, self-referential state.

mod errors;
mod model;
mod public;
mod validation;

pub use errors::*;
pub use model::*;

#[cfg(test)]
mod tests;
