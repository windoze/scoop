use std::collections::HashSet;

use scoop_identity::{AccessorRole, PropertyOwner as PersistentPropertyOwner};

use super::{ExportPropertyAccessorBuildError, PropertyInterfaceBuildError};
use crate::{
    DeclaredVisibility, ExportHir, PropertyAccessorImplementation, PropertyCapability,
    PropertyGetter, PropertyGetterId, PropertyOwner, PropertyPublicAccessV1,
    PropertyRepresentation, PropertyRepresentationV1, PropertySetter, PropertySetterId,
    PropertySetterPublicAccessV1,
};

pub(super) struct ProjectedAccessors<'a> {
    pub(super) getter_id: scoop_identity::PersistentPropertyAccessorId,
    pub(super) getter: &'a PropertyGetter,
    pub(super) setter: Option<ProjectedSetter<'a>>,
}

#[derive(Clone, Copy)]
pub(super) struct ProjectedSetter<'a> {
    pub(super) id: scoop_identity::PersistentPropertyAccessorId,
    pub(super) declaration: &'a PropertySetter,
    pub(super) access: PropertySetterPublicAccessV1,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project_accessors<'a>(
    export: &'a ExportHir,
    property_id: crate::PropertyId,
    property: PersistentPropertyOwner,
    capability: PropertyCapability,
    expected_access: PropertyPublicAccessV1,
    public_getters: &HashSet<PropertyGetterId>,
    public_setters: &HashSet<PropertySetterId>,
) -> Result<ProjectedAccessors<'a>, PropertyInterfaceBuildError> {
    let getter_id = capability.getter();
    let getter = arena_get(&export.property_getters, getter_id).ok_or_else(|| {
        accessor_error(
            property,
            AccessorRole::Getter,
            ExportPropertyAccessorBuildError::Unknown(raw_index(getter_id)),
        )
    })?;
    let identity = export
        .property_accessor_identities
        .get_getter(getter_id)
        .ok_or_else(|| {
            accessor_error(
                property,
                AccessorRole::Getter,
                ExportPropertyAccessorBuildError::MissingIdentity(raw_index(getter_id)),
            )
        })?;
    validate_accessor_identity(property_id, property, AccessorRole::Getter, identity)?;
    validate_public_accessor(
        property,
        AccessorRole::Getter,
        &getter.access,
        expected_access,
        public_getters.contains(&getter_id),
    )?;

    let setter = capability
        .setter()
        .map(|setter_id| {
            project_setter(
                export,
                property_id,
                property,
                setter_id,
                expected_access,
                public_setters,
            )
        })
        .transpose()?;
    Ok(ProjectedAccessors {
        getter_id: identity.id(),
        getter,
        setter,
    })
}

fn project_setter<'a>(
    export: &'a ExportHir,
    property_id: crate::PropertyId,
    property: PersistentPropertyOwner,
    setter_id: PropertySetterId,
    expected_access: PropertyPublicAccessV1,
    public_setters: &HashSet<PropertySetterId>,
) -> Result<ProjectedSetter<'a>, PropertyInterfaceBuildError> {
    let setter = arena_get(&export.property_setters, setter_id).ok_or_else(|| {
        accessor_error(
            property,
            AccessorRole::Setter,
            ExportPropertyAccessorBuildError::Unknown(raw_index(setter_id)),
        )
    })?;
    let identity = export
        .property_accessor_identities
        .get_setter(setter_id)
        .ok_or_else(|| {
            accessor_error(
                property,
                AccessorRole::Setter,
                ExportPropertyAccessorBuildError::MissingIdentity(raw_index(setter_id)),
            )
        })?;
    validate_accessor_identity(property_id, property, AccessorRole::Setter, identity)?;

    let is_in_public_surface = public_setters.contains(&setter_id);
    let access = match project_public_access(&setter.access) {
        Some(actual) => {
            if !is_in_public_surface {
                return Err(accessor_error(
                    property,
                    AccessorRole::Setter,
                    ExportPropertyAccessorBuildError::MissingPublicSurface,
                ));
            }
            if actual != expected_access {
                return Err(accessor_error(
                    property,
                    AccessorRole::Setter,
                    ExportPropertyAccessorBuildError::AccessMismatch {
                        expected: expected_access,
                        actual,
                    },
                ));
            }
            PropertySetterPublicAccessV1::Public
        }
        None => {
            if is_in_public_surface {
                return Err(accessor_error(
                    property,
                    AccessorRole::Setter,
                    ExportPropertyAccessorBuildError::UnexpectedPublicSurface,
                ));
            }
            PropertySetterPublicAccessV1::Restricted
        }
    };
    Ok(ProjectedSetter {
        id: identity.id(),
        declaration: setter,
        access,
    })
}

fn validate_public_accessor(
    property: PersistentPropertyOwner,
    role: AccessorRole,
    access: &crate::DeclarationAccess,
    expected_access: PropertyPublicAccessV1,
    is_in_public_surface: bool,
) -> Result<(), PropertyInterfaceBuildError> {
    if !is_in_public_surface {
        return Err(accessor_error(
            property,
            role,
            ExportPropertyAccessorBuildError::MissingPublicSurface,
        ));
    }
    let actual_access = project_public_access(access).ok_or_else(|| {
        accessor_error(property, role, ExportPropertyAccessorBuildError::NotPublic)
    })?;
    if actual_access != expected_access {
        return Err(accessor_error(
            property,
            role,
            ExportPropertyAccessorBuildError::AccessMismatch {
                expected: expected_access,
                actual: actual_access,
            },
        ));
    }
    Ok(())
}

fn validate_accessor_identity(
    property_id: crate::PropertyId,
    property: PersistentPropertyOwner,
    role: AccessorRole,
    identity: &crate::HirPropertyAccessorIdentity,
) -> Result<(), PropertyInterfaceBuildError> {
    if identity.property() != property_id {
        return Err(accessor_error(
            property,
            role,
            ExportPropertyAccessorBuildError::PropertyMismatch {
                actual: raw_index(identity.property()),
            },
        ));
    }
    if identity.record().key().owner() != property {
        return Err(accessor_error(
            property,
            role,
            ExportPropertyAccessorBuildError::PersistentOwnerMismatch,
        ));
    }
    if identity.record().key().role() != role {
        return Err(accessor_error(
            property,
            role,
            ExportPropertyAccessorBuildError::RoleMismatch {
                actual: identity.record().key().role(),
            },
        ));
    }
    Ok(())
}

pub(super) fn project_public_access(
    access: &crate::DeclarationAccess,
) -> Option<PropertyPublicAccessV1> {
    if access.declared != DeclaredVisibility::Public || !access.lookup.0.is_universal() {
        return None;
    }
    match &access.slot {
        Some(slot) if slot.0.is_universal() => Some(PropertyPublicAccessV1::PublicSlot),
        Some(_) => None,
        None => Some(PropertyPublicAccessV1::DirectOnly),
    }
}

pub(super) fn project_representation(
    property: &crate::Property,
    getter: &PropertyGetter,
    setter: Option<&PropertySetter>,
) -> Result<PropertyRepresentationV1, &'static str> {
    if matches!(
        property.representation,
        PropertyRepresentation::Const { .. }
    ) {
        if !matches!(
            property.owner,
            PropertyOwner::TopLevel | PropertyOwner::Object(_)
        ) {
            return Err("const property owner is neither top-level nor object");
        }
        if !matches!(
            getter.implementation,
            PropertyAccessorImplementation::Constant
        ) {
            return Err("const property getter is not a constant accessor");
        }
        return Ok(PropertyRepresentationV1::Const);
    }
    let all_abstract = std::iter::once(getter.implementation)
        .chain(setter.map(|setter| setter.implementation))
        .all(|implementation| {
            matches!(
                implementation,
                PropertyAccessorImplementation::AbstractSlot(_)
            )
        });
    if all_abstract {
        Ok(PropertyRepresentationV1::AbstractSlot)
    } else {
        Ok(PropertyRepresentationV1::RuntimeAccessor)
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

fn accessor_error(
    property: PersistentPropertyOwner,
    role: AccessorRole,
    detail: ExportPropertyAccessorBuildError,
) -> PropertyInterfaceBuildError {
    PropertyInterfaceBuildError::Accessor {
        property,
        role,
        detail,
    }
}
