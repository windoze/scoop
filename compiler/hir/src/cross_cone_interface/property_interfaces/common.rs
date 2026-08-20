mod access;
mod accessors;
mod capability;
mod errors;

pub use access::{PropertyPublicAccessV1, PropertyRepresentationV1, PropertySetterPublicAccessV1};
pub use accessors::{
    DecodedPropertyAccessorsV1, PropertyAccessorImplementationV1, PropertyAccessorSourceV1,
    PropertyAccessorsV1,
};
pub use capability::{DecodedPropertyCapabilityV1, PropertyCapabilityV1};
pub use errors::{PropertyCapabilityBuildError, PropertyCapabilityResolutionError};

#[cfg(test)]
mod tests;
