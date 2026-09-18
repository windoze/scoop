use scoop_identity::{AccessorRole, CallableTemplateOrigin, PropertyOwner, SignatureTypeKey};

use super::{
    CallableInterfaceBuildError, CallableProjection, CallableProjectionError,
    CallableProjectionSubject, SourceParameterProjectionError, access, effects,
};
use crate::{
    CallableInterfaceRecordV1, CallableModalityV1, CanonicalSourceParameterShapesV1,
    HirPropertyIdentity, MethodModifier, PropertyAccessorImplementation, PropertyPublicAccessV1,
    PropertySetterPublicAccessV1, PublicLookupAccessV1, SourceParameterShapeV1,
};

pub(super) fn project_all(
    projection: &CallableProjection<'_>,
    records: &mut Vec<CallableInterfaceRecordV1>,
) -> Result<(), CallableInterfaceBuildError> {
    for &getter in &projection.export.public_surface.property_getters {
        let subject = CallableProjectionSubject::Getter(super::raw_index(getter));
        records.push(
            project_getter(projection, getter)
                .map_err(|error| CallableInterfaceBuildError::projection(subject, error))?,
        );
    }
    for &setter in &projection.export.public_surface.property_setters {
        let subject = CallableProjectionSubject::Setter(super::raw_index(setter));
        records.push(
            project_setter(projection, setter)
                .map_err(|error| CallableInterfaceBuildError::projection(subject, error))?,
        );
    }
    Ok(())
}

fn project_getter(
    projection: &CallableProjection<'_>,
    getter_id: crate::PropertyGetterId,
) -> Result<CallableInterfaceRecordV1, CallableProjectionError> {
    let getter = super::arena_get(&projection.export.property_getters, getter_id)
        .ok_or(CallableProjectionError::UnknownDeclaration)?;
    let identity = projection
        .export
        .property_accessor_identities
        .get_getter(getter_id)
        .ok_or(CallableProjectionError::MissingIdentity)?;
    let (property, interface) =
        property_interface(projection, identity, AccessorRole::Getter, identity.id())?;
    let parameters = CanonicalSourceParameterShapesV1::try_new(Vec::new()).map_err(|source| {
        CallableProjectionError::Parameters(SourceParameterProjectionError::List(source))
    })?;
    finish(
        projection,
        property,
        interface,
        identity.id(),
        getter.access.clone(),
        getter.attributes,
        getter.implementation,
        parameters,
        interface.value_type().clone(),
    )
}

fn project_setter(
    projection: &CallableProjection<'_>,
    setter_id: crate::PropertySetterId,
) -> Result<CallableInterfaceRecordV1, CallableProjectionError> {
    let setter = super::arena_get(&projection.export.property_setters, setter_id)
        .ok_or(CallableProjectionError::UnknownDeclaration)?;
    let identity = projection
        .export
        .property_accessor_identities
        .get_setter(setter_id)
        .ok_or(CallableProjectionError::MissingIdentity)?;
    let (property, interface) =
        property_interface(projection, identity, AccessorRole::Setter, identity.id())?;
    if interface.capability().setter_access() != Some(PropertySetterPublicAccessV1::Public) {
        return Err(CallableProjectionError::PropertyOwnerMismatch);
    }
    let name =
        scoop_identity::CanonicalIdentifier::new(&setter.parameter_name).map_err(|source| {
            CallableProjectionError::Parameters(SourceParameterProjectionError::InvalidName {
                position: 0,
                source,
            })
        })?;
    let parameters = CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
        name,
        interface.value_type().clone(),
    )])
    .map_err(|source| {
        CallableProjectionError::Parameters(SourceParameterProjectionError::List(source))
    })?;
    finish(
        projection,
        property,
        interface,
        identity.id(),
        setter.access.clone(),
        setter.attributes,
        setter.implementation,
        parameters,
        SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn finish(
    projection: &CallableProjection<'_>,
    property: &crate::Property,
    interface: &crate::PropertyInterfaceRecordV1,
    accessor: scoop_identity::PersistentPropertyAccessorId,
    declaration_access: crate::DeclarationAccess,
    attributes: crate::FunctionAttributes,
    implementation: PropertyAccessorImplementation,
    parameters: CanonicalSourceParameterShapesV1,
    result: SignatureTypeKey,
) -> Result<CallableInterfaceRecordV1, CallableProjectionError> {
    let type_parameters = projection
        .signatures
        .project_binder_list(&[], &[])
        .map_err(CallableProjectionError::Signature)?;
    let actual_access =
        access::project(&declaration_access).map_err(CallableProjectionError::Access)?;
    let expected_access = match interface.access() {
        PropertyPublicAccessV1::DirectOnly => PublicLookupAccessV1::DirectOnly,
        PropertyPublicAccessV1::PublicSlot => PublicLookupAccessV1::PublicSlot,
    };
    if actual_access != expected_access {
        return Err(CallableProjectionError::AccessorAccessMismatch {
            expected: expected_access,
            actual: actual_access,
        });
    }
    let effects = effects::accessor(attributes).map_err(CallableProjectionError::Effects)?;
    CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Accessor(accessor),
        interface.owner(),
        type_parameters,
        interface.receiver().cloned(),
        parameters,
        result,
        effects,
        modality(property, implementation),
        actual_access,
    )
    .map_err(CallableProjectionError::Record)
}

fn property_interface<'a>(
    projection: &'a CallableProjection<'_>,
    identity: &crate::HirPropertyAccessorIdentity,
    expected_role: AccessorRole,
    accessor: scoop_identity::PersistentPropertyAccessorId,
) -> Result<(&'a crate::Property, &'a crate::PropertyInterfaceRecordV1), CallableProjectionError> {
    if identity.record().key().role() != expected_role {
        return Err(CallableProjectionError::AccessorRole {
            expected: expected_role,
            actual: identity.record().key().role(),
        });
    }
    let property = super::arena_get(&projection.export.properties, identity.property())
        .ok_or(CallableProjectionError::PropertyOwnerMismatch)?;
    let declaration = projection
        .export
        .property_identities
        .get(identity.property())
        .map(persistent_property_owner)
        .ok_or(CallableProjectionError::PropertyOwnerMismatch)?;
    if identity.record().key().owner() != declaration {
        return Err(CallableProjectionError::PropertyOwnerMismatch);
    }
    let interface = projection.properties.get(declaration).ok_or(
        CallableProjectionError::MissingPropertyInterface(declaration),
    )?;
    let expected_accessor = match expected_role {
        AccessorRole::Getter => Some(interface.capability().getter()),
        AccessorRole::Setter => interface.capability().setter(),
    };
    if expected_accessor != Some(accessor) {
        return Err(CallableProjectionError::PropertyOwnerMismatch);
    }
    Ok((property, interface))
}

fn persistent_property_owner(identity: &HirPropertyIdentity) -> PropertyOwner {
    match identity {
        HirPropertyIdentity::Ordinary(record) => PropertyOwner::Property(record.id()),
        HirPropertyIdentity::Extension(record) => PropertyOwner::ExtensionProperty(record.id()),
    }
}

fn modality(
    property: &crate::Property,
    implementation: PropertyAccessorImplementation,
) -> CallableModalityV1 {
    match implementation {
        PropertyAccessorImplementation::AbstractSlot(_) => CallableModalityV1::Abstract,
        PropertyAccessorImplementation::Body(_)
            if matches!(property.owner, crate::PropertyOwner::Interface(_)) =>
        {
            CallableModalityV1::InterfaceDefault
        }
        PropertyAccessorImplementation::Storage
        | PropertyAccessorImplementation::Constant
        | PropertyAccessorImplementation::Body(_) => match property.modifier {
            MethodModifier::Final => CallableModalityV1::Final,
            MethodModifier::Open => CallableModalityV1::Open,
            MethodModifier::Abstract => CallableModalityV1::Abstract,
        },
    }
}
