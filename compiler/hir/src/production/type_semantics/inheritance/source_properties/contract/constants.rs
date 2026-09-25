use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    export: &ExportHir,
    property: &Property,
    id: PersistentPropertyId,
    value: &ConstPropertyValue,
    value_type: SignatureTypeKey,
    access: &DeclarationAccessSourceV1,
) -> Result<ExportConstValueV1, Error> {
    if !matches!(property.owner, PropertyOwner::Object(_))
        || !matches!(property.capability, PropertyCapability::ReadOnly { .. })
        || property.access.slot.is_some()
    {
        return Err(invalid(
            "nominal const source must be a direct read-only object property",
        ));
    }

    let value = CanonicalConstValueV1::from(value.clone());
    let expected = match export.types[property.ty] {
        Type::Integer(kind) => CanonicalConstValueKindV1::Integer(kind),
        Type::Boolean => CanonicalConstValueKindV1::Boolean,
        Type::String => CanonicalConstValueKindV1::String,
        _ => return Err(invalid("nominal const source has a nonconstant value type")),
    };
    if value.kind() != expected {
        return Err(invalid(
            "nominal const source value differs from its declared type",
        ));
    }

    Ok(ExportConstValueV1::new(
        id,
        value_type,
        value,
        access.definition_origin().clone(),
    ))
}
