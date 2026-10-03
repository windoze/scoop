use super::*;

pub(super) fn validate_declaration_shape(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: &CanonicalBinderListV1,
    has_receiver: bool,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    match declaration {
        PropertyOwner::Property(_) => {
            if owner == PublicDeclarationOwnerV1::Extension {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedExtensionOwner(
                    declaration,
                ));
            }
            if !type_parameters.is_empty() {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedTypeParameters(
                    declaration,
                ));
            }
            if has_receiver {
                return Err(PropertyInterfaceRecordBuildError::UnexpectedReceiver(owner));
            }
        }
        PropertyOwner::ExtensionProperty(_) => {
            if owner != PublicDeclarationOwnerV1::Extension {
                return Err(PropertyInterfaceRecordBuildError::ExtensionOwnerRequired {
                    declaration,
                    actual: owner,
                });
            }
            if !has_receiver {
                return Err(PropertyInterfaceRecordBuildError::MissingExtensionReceiver);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_access_shape(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    access: PropertyPublicAccessV1,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    if !matches!(owner, PublicDeclarationOwnerV1::Nominal(_))
        && access != PropertyPublicAccessV1::DirectOnly
    {
        return Err(PropertyInterfaceRecordBuildError::DirectPropertyContract {
            declaration,
            owner,
            access,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_source_representation(
    declaration: PropertyDeclarationId,
    owner: PublicDeclarationOwnerV1,
    type_parameters: &CanonicalBinderListV1,
    has_receiver: bool,
    accessors: PropertyAccessorsV1,
    representation: PropertyRepresentationV1,
) -> Result<(), PropertyInterfaceRecordBuildError> {
    match representation {
        PropertyRepresentationV1::Const => {
            if !accessors.is_read_only() {
                return Err(PropertyInterfaceRecordBuildError::ConstMustBeReadOnly(
                    declaration,
                ));
            }
            if !type_parameters.is_empty() {
                return Err(PropertyInterfaceRecordBuildError::ConstCannotBeGeneric(
                    declaration,
                ));
            }
            if has_receiver {
                return Err(PropertyInterfaceRecordBuildError::ConstCannotHaveReceiver(
                    declaration,
                ));
            }
        }
        PropertyRepresentationV1::AbstractSlot => {
            if !matches!(owner, PublicDeclarationOwnerV1::Nominal(_)) {
                return Err(
                    PropertyInterfaceRecordBuildError::AbstractNominalOwnerRequired {
                        declaration,
                        actual: owner,
                    },
                );
            }
        }
        PropertyRepresentationV1::RuntimeAccessor => {}
    }
    let forms = || {
        std::iter::once(accessors.getter_source())
            .chain(accessors.setter_source())
            .map(|source| source.implementation())
    };
    use crate::PropertyAccessorImplementationV1 as Form;
    let valid = match representation {
        PropertyRepresentationV1::Const => {
            accessors.is_read_only() && accessors.getter_source().implementation() == Form::Constant
        }
        PropertyRepresentationV1::AbstractSlot => forms().all(|form| form == Form::AbstractSlot),
        PropertyRepresentationV1::RuntimeAccessor => {
            forms().all(|form| form != Form::Constant)
                && forms().any(|form| form != Form::AbstractSlot)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(PropertyInterfaceRecordBuildError::AccessorImplementations(
            declaration,
        ))
    }
}
