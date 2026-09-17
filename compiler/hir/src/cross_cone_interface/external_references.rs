//! Exact foreign-reference closure carried by the cross-Cone HIR interface.

mod record;
mod roles;
mod table;
mod target;
mod witness;

pub use record::{
    DecodedExternalHirReferenceV1, ExternalHirReferenceBuildError,
    ExternalHirReferenceResolutionError, ExternalHirReferenceResolver,
    ExternalHirReferenceSemanticAuthority, ExternalHirReferenceSemanticValidationError,
    ExternalHirReferenceV1,
};
pub use roles::{
    CanonicalExternalHirReferenceRolesV1, DecodedCanonicalExternalHirReferenceRolesV1,
    ExternalHirReferenceRoleSetBuildError, ExternalHirReferenceRoleSetValidationError,
    ExternalHirReferenceRoleV1,
};
pub use table::{
    CanonicalExternalHirReferencesV1, DecodedCanonicalExternalHirReferencesV1,
    ExternalHirReferenceSetBuildError, ExternalHirReferenceSetSemanticValidationError,
    ExternalHirReferenceSetValidationError,
};
pub use target::{
    DecodedExternalHirTargetV1, ExternalHirTargetResolutionError, ExternalHirTargetResolver,
    ExternalHirTargetV1,
};
pub use witness::{
    CanonicalDependencyBindingWitnessesV1, DecodedCanonicalDependencyBindingWitnessesV1,
    DecodedDependencyBindingWitnessV1, DependencyBindingWitnessResolutionError,
    DependencyBindingWitnessSemanticValidationError, DependencyBindingWitnessSetBuildError,
    DependencyBindingWitnessSetValidationError, DependencyBindingWitnessV1,
};

#[cfg(test)]
mod test_support;
