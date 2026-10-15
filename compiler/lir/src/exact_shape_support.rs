//! Physical shape-support roles shared by ordinary and core producers.

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
