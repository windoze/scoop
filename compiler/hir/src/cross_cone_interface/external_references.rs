//! Exact foreign-reference closure carried by the cross-Cone HIR interface.

mod roles;
mod target;

pub use roles::{
    CanonicalExternalHirReferenceRolesV1, DecodedCanonicalExternalHirReferenceRolesV1,
    ExternalHirReferenceRoleSetBuildError, ExternalHirReferenceRoleSetValidationError,
    ExternalHirReferenceRoleV1,
};
pub use target::{
    DecodedExternalHirTargetV1, ExternalHirTargetResolutionError, ExternalHirTargetResolver,
    ExternalHirTargetV1,
};
