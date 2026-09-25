use super::*;
use crate::{
    CanonicalConstValueKindV1, ExportConstValueV1, IntrinsicTypeKind, NominalSourceShapeV1,
    PublicNominalKindV1,
};
use scoop_identity::SignatureTypeKey;

pub(super) fn validate(
    types: MetadataTypes<'_, '_>,
    record: &NominalSupportPropertyInterfaceV1,
    value: &ExportConstValueV1,
) -> Result<(), Error> {
    let metadata = types.current;
    let id = record.declaration();
    let source = property(metadata, id)?;
    let expected_access = property_access(metadata, id)?;

    if source.owner().nominal_owner() != Some(record.owner())
        || source.representation() != crate::PropertyRepresentationV1::Const
        || record.declaration_access() != &expected_access
        || value.property() != id
        || value.value_type() != source.value_type()
        || value.definition_origin() != expected_access.definition_origin()
    {
        return Err(Error::PropertyContract(id));
    }
    let table = metadata.public.nominal_interfaces();

    if table
        .declaration(record.owner())
        .is_none_or(|owner| owner.kind() != PublicNominalKindV1::Object)
    {
        return Err(Error::PropertyContract(id));
    }
    let SignatureTypeKey::Nominal(value_type) = value.value_type() else {
        return Err(Error::PropertyContract(id));
    };
    let nominal = types.nominal(*value_type)?;
    let expected = match value.value().kind() {
        CanonicalConstValueKindV1::Integer(kind) => IntrinsicTypeKind::Integer(kind),
        CanonicalConstValueKindV1::Boolean => IntrinsicTypeKind::Boolean,
        CanonicalConstValueKindV1::String => IntrinsicTypeKind::String,
    };
    if !matches!(nominal.source_shape(), NominalSourceShapeV1::Intrinsic(actual) if actual.family() == expected)
    {
        return Err(Error::PropertyContract(id));
    }
    let properties = metadata.public.property_interfaces();

    if properties.get(PropertyOwner::Property(id)).is_some() {
        let constants = metadata.public.constants();

        let expected = constants.get(id).ok_or(Error::PropertyContract(id))?;

        if value != expected {
            return Err(Error::PropertyContract(id));
        }
    }
    Ok(())
}
