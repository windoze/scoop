//! Ordinary-producer physical shape-support closures.
//!
//! The eight-role wire product is shared with the frozen core authority. This
//! constituent only admits ordinary producers; core continues to use its
//! existing strong-production branch.

mod error;
pub use error::*;

mod model;
pub use model::*;

mod replay;

mod table;
pub use table::*;

mod wire;
pub use wire::*;

#[cfg(test)]
mod tests;
