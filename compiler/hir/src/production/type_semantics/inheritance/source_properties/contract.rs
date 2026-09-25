use super::*;
use crate::production::nominal_interfaces::owner_resolution;
use scoop_identity::{DefinitionOwnerAtom, SignatureTypeKey};

mod constants;

pub(super) fn project(
    export: &ExportHir,
    id: PropertyId,
    signatures: &HirInterfaceSignatureProjector<'_>,
) -> Result<NominalSupportPropertyInterfaceV1, Error> {
    let property = &export.properties[id];
    let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
        return Err(invalid("nominal source property has an extension identity"));
    };
    let key = identity.key();
    if key.origin() != export.cone {
        return Err(invalid("source property belongs to another cone"));
    }
    let parameters = match property.owner {
        PropertyOwner::Class(id) => export.classes[id].type_params.as_slice(),
        PropertyOwner::Interface(id) => export.interfaces[id].type_params.as_slice(),
        PropertyOwner::Struct(id) => export.structs[id].type_params.as_slice(),
        PropertyOwner::Enum(id) => export.enums[id].type_params.as_slice(),
        PropertyOwner::Object(_) => &[],
        PropertyOwner::TopLevel | PropertyOwner::Extension(_) => {
            return Err(invalid("source property has no nominal owner"));
        }
    };
    let owner = owner_resolution::from_property(export, property.owner)
        .ok_or_else(|| invalid("source property has no nominal source identity"))?;
    let owner_atom = match owner {
        SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(id),
        SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    };
    if key.owners().owners().last() != Some(&owner_atom) {
        return Err(invalid("source property has a different lexical owner"));
    }

    let binders = signatures.binder_frame(parameters, 0).map_err(invalid)?;

    let value_type = signatures
        .map_type(property.ty, &binders)
        .map_err(invalid)?;
    let access = declaration_access(
        export,
        key,
        DefinitionOriginSubject::Property(identity.id()),
        property.access.declared,
    )?;
    let payload = match &property.representation {
        PropertyRepresentation::Const { value } => NominalSupportPropertyPayloadV1::Const {
            value: constants::project(export, property, identity.id(), value, value_type, &access)?,
        },
        _ => NominalSupportPropertyPayloadV1::Runtime {
            interface: runtime(export, property, key, owner, value_type)?,
        },
    };
    NominalSupportPropertyInterfaceV1::try_new(identity.id(), access, payload).map_err(invalid)
}

fn runtime(
    export: &ExportHir,
    property: &Property,
    key: &SourceDeclarationKey,
    owner: SourceNominalId,
    value_type: SignatureTypeKey,
) -> Result<NominalSourcePropertyPayloadV1, Error> {
    let getter_id = property.capability.getter();
    let getter = &export.property_getters[getter_id];
    let setter = property
        .capability
        .setter()
        .map(|id| &export.property_setters[id]);
    let representation =
        source_property_representation(property, getter, setter).map_err(invalid)?;
    let mutability = match property.capability.setter() {
        None => ProtectedPropertyMutabilityV1::ReadOnly,
        Some(setter_id) => {
            let setter = &export.property_setters[setter_id];
            let id = export.property_accessor_identities[setter_id].id();
            ProtectedPropertyMutabilityV1::ReadWrite {
                setter: id,
                setter_access: declaration_access(
                    export,
                    key,
                    DefinitionOriginSubject::PropertyAccessor(id),
                    setter.access.declared,
                )?,
            }
        }
    };
    NominalSourcePropertyPayloadV1::try_new(
        owner,
        value_type,
        export.property_accessor_identities[getter_id].id(),
        mutability,
        representation,
        slots::project(
            export,
            getter.implementation,
            setter.map(|setter| setter.implementation),
        )?,
    )
    .map_err(invalid)
}
