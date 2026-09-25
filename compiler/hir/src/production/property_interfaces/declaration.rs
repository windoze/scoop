use super::*;
use crate::{
    PropertyAccessorImplementationV1, PropertyAccessorSourceV1, PropertyAccessorsV1,
    PropertyDeclarationRecordV1, PropertyId,
};
use scoop_identity::AccessorRole;

pub(super) fn project(
    export: &ExportHir,
    projector: &HirInterfaceSignatureProjector<'_>,
    property_id: PropertyId,
) -> Result<PropertyDeclarationRecordV1, PropertyInterfaceBuildError> {
    use PropertyInterfaceBuildError as Error;
    let property = arena_get(&export.properties, property_id)
        .ok_or_else(|| Error::UnknownPublicProperty(raw_index(property_id)))?;
    let identity = export
        .property_identities
        .get(property_id)
        .ok_or_else(|| Error::MissingPropertyIdentity(raw_index(property_id)))?;
    let declaration = persistent_property_owner(identity);
    signature::validate_declaration_identity(export, declaration, identity)?;
    let signature =
        signature::project_property_signature(export, projector, property_id, property)?;
    let accessors =
        accessors::project_accessors(export, property_id, declaration, property.capability)?;
    if accessors.getter.access.declared != property.access.declared {
        return Err(Error::Accessor {
            property: declaration,
            role: AccessorRole::Getter,
            detail: ExportPropertyAccessorBuildError::DeclaredVisibilityMismatch {
                expected: property.access.declared,
                actual: accessors.getter.access.declared,
            },
        });
    }
    let getter = PropertyAccessorSourceV1::new(
        accessors.getter_id,
        PropertyAccessorImplementationV1::from_source(accessors.getter.implementation),
    );
    let identities = match accessors.setter {
        None => PropertyAccessorsV1::read_only(getter),
        Some(setter) => PropertyAccessorsV1::try_read_write(
            getter,
            PropertyAccessorSourceV1::new(
                setter.id,
                PropertyAccessorImplementationV1::from_source(setter.declaration.implementation),
            ),
        )
        .map_err(|source| Error::Capability {
            property: declaration,
            source,
        })?,
    };
    let representation = accessors::project_representation(
        property,
        accessors.getter,
        accessors.setter.map(|setter| setter.declaration),
    )
    .map_err(|detail| Error::Representation {
        property: declaration,
        detail,
    })?;
    let value_type = projector
        .map_type(property.ty, &signature.binders)
        .map_err(|source| Error::Signature {
            property: declaration,
            source,
        })?;
    PropertyDeclarationRecordV1::try_new(
        declaration,
        signature.owner,
        signature.type_parameters,
        signature.receiver,
        value_type,
        identities,
        representation,
        property.access.declared.into(),
    )
    .map_err(|source| Error::Record {
        property: declaration,
        source,
    })
}
