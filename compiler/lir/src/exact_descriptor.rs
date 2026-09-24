//! Complete physical TypeDescriptor records.
//!
//! A record joins target layouts, Strong V2 registration semantics and the
//! canonical diagnostic spelling. The containing layout/ABI section still
//! owns source-export and selected-dependency authority.

mod error;
pub use error::*;

mod model;
pub use model::*;

mod constituents;
pub use constituents::ExactDescriptorSourceInputV1;
mod resources;

mod replay;
pub(crate) use replay::{validate_inline_scan, validate_registration_plan};

mod table;
pub use table::*;

mod wire;
pub use wire::{
    DecodedCanonicalExactDescriptorExportsV1, DecodedExactDescriptorExportV1,
    DecodedExactDescriptorSemanticProjectionV1, ExactDescriptorSemanticProjectionV1,
    ExactDescriptorWireError,
};

#[cfg(test)]
mod tests;
