use std::fmt;

use scoop_hir as hir;

use crate::{Lowerer, persistent_types::identity_inputs};

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    callback_core: hir::ForeignCallbackCore,
    nominals: &hir::HirNominalIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    constructors: &hir::HirConstructorIdentities,
    enum_members: &hir::HirEnumMemberIdentities,
    functions: &hir::HirFunctionIdentities,
    intrinsic_core: &hir::IntrinsicTypeCore,
) -> Result<hir::HirCallbackRegistrationIdentities, PersistentCallbackIdentityError> {
    hir::HirCallbackRegistrationIdentities::from_registrations(
        hir::HirCallbackRegistrationIdentityInputs {
            registrations: &lowerer.foreign_callback_registrations,
            functions: &lowerer.functions,
            lambdas: &lowerer.lambdas,
            anonymous_functions: &lowerer.anonymous_functions,
            local_functions: &lowerer.local_functions,
            class_constructors: &lowerer.class_constructors,
            struct_constructors: &lowerer.struct_constructors,
            function_identities: functions,
            property_accessor_identities: accessors,
            constructor_identities: constructors,
            enum_member_identities: enum_members,
            callback_modes: callback_core.modes,
            type_inputs: identity_inputs(lowerer, nominals, intrinsic_core),
            unit: lowerer.unit,
        },
    )
    .map_err(PersistentCallbackIdentityError)
}

#[derive(Debug)]
pub(crate) struct PersistentCallbackIdentityError(hir::HirCallbackRegistrationIdentityError);

impl PersistentCallbackIdentityError {
    pub(crate) const fn registration(&self) -> Option<hir::ForeignCallbackRegistrationId> {
        self.0.registration()
    }
}

impl fmt::Display for PersistentCallbackIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive callback registration identity: {}",
            self.0
        )
    }
}

impl std::error::Error for PersistentCallbackIdentityError {}
