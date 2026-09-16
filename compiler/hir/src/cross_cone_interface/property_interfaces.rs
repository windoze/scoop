mod common;
mod record;

pub use common::{
    DecodedPropertyCapabilityV1, PropertyCapabilityBuildError, PropertyCapabilityResolutionError,
    PropertyCapabilityV1, PropertyPublicAccessV1, PropertyRepresentationV1,
    PropertySetterPublicAccessV1,
};
pub use record::{
    DecodedPropertyInterfaceRecordV1, PropertyDeclarationIdentityShapeV1,
    PropertyDeclarationSourceShapeV1, PropertyInterfaceRecordBuildError,
    PropertyInterfaceRecordResolutionError, PropertyInterfaceRecordResolver,
    PropertyInterfaceRecordV1, PropertyInterfaceSemanticAuthority,
    PropertyInterfaceSemanticValidationError,
};
