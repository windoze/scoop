//! Exact foreign-reference closure carried by the cross-Cone HIR interface.

mod roles;

pub use roles::{
    CanonicalExternalHirReferenceRolesV1, DecodedCanonicalExternalHirReferenceRolesV1,
    ExternalHirReferenceRoleSetBuildError, ExternalHirReferenceRoleSetValidationError,
    ExternalHirReferenceRoleV1,
};
