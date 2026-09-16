mod access;
mod capability;
mod errors;

pub use access::{PropertyPublicAccessV1, PropertyRepresentationV1, PropertySetterPublicAccessV1};
pub use capability::{DecodedPropertyCapabilityV1, PropertyCapabilityV1};
pub use errors::{PropertyCapabilityBuildError, PropertyCapabilityResolutionError};

#[cfg(test)]
mod tests;
