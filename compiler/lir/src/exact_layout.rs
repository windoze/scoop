//! Complete physical layout records and their checked dependencies.
//!
//! These records do not authorize an export or import. Source representation
//! joins and selected-closure ownership remain separate section obligations.
//! In particular, scalar, string, and array families must be joined to the
//! trusted-core intrinsic bindings before a record enters a complete section.
//! Only Unit has a compiler-fixed nominal identity checked by this constituent.

mod identity;
pub use identity::*;

mod error;
pub use error::*;

mod model;
pub use model::*;

mod replay;
pub use replay::*;

mod wire;
pub use wire::{
    DecodedExactLayoutExportV1, DecodedExactLayoutSemanticProjectionV1,
    ExactLayoutSemanticProjectionV1, ExactLayoutWireError,
};

mod table;
pub use table::*;

#[cfg(test)]
pub(crate) mod tests;
