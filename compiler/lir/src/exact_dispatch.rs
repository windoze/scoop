//! Complete physical dispatch-table records.
//!
//! A record joins one validated MIR dispatch schema to the emitted LIR table,
//! callable ABI exports, and the Strong table definition. The containing
//! section resolves actual typed dependency references.

mod callables;
pub use callables::DispatchCallableAbiV1;

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
