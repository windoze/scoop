//! Complete physical layouts and references to their actual definitions.
//! Intrinsic representation is resolved by the frontend and retained in typed
//! MIR. Layout consumers use the same records for every declaration provider.

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
