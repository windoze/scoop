//! Complete physical dispatch-table records.
//!
//! A record joins one validated MIR dispatch schema to the emitted LIR table,
//! exact callable ABI exports, and the Strong table definition. The containing
//! section remains responsible for source authority and selected dependency
//! closure.

mod error;
pub use error::*;

mod model;
pub use model::*;

mod physical;
pub use physical::*;

mod replay;

mod table;
pub use table::*;

mod wire;
pub use wire::{
    DecodedCanonicalExactDispatchExportsV1, DecodedExactDispatchExportV1,
    DecodedExactDispatchSemanticProjectionV1, ExactDispatchSemanticProjectionV1,
    ExactDispatchWireError,
};

#[cfg(test)]
mod tests;
