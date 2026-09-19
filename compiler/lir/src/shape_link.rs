//! Semantic physical-import contracts replayed from terminal provider plans.
//! This constituent does not grant complete Link or Compile selection.

mod contract;
mod decoded;
mod error;
mod import;
mod provider;
pub(crate) mod support;
mod table;
mod wire;

pub use contract::{DecodedShapeLinkContractV1, ShapeLinkContractV1};
pub use decoded::DecodedExternalShapeLinkImportV1;
pub use error::ShapeLinkError;
pub use import::ExternalShapeLinkImportV1;
pub use provider::{ShapeLinkProductionV1, ShapeLinkProviderPartsV1, ShapeLinkProviderV1};
pub use support::{ShapeLinkSupportAuthorityV1, ShapeLinkSupportSourceV1};
pub use table::{CanonicalExternalShapeLinkImportsV1, DecodedCanonicalExternalShapeLinkImportsV1};

#[cfg(test)]
mod tests;
