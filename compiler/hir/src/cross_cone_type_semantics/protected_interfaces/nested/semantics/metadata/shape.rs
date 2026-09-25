use super::*;
use scoop_identity::{
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentObjectValueId,
};

pub(super) fn validate<A: NestedNominalSemanticAuthority<E>, E>(
    owner: SourceNominalId,
    source: &ProtectedNestedSourceInterfaceV1,
    authority: &mut A,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    let scope = source.type_parameters().signature_scope(None);
    let field_type = |ty: &SignatureTypeKey, authority: &mut A| {
        scope
            .validate_signature_semantics(ty, authority)
            .map_err(Error::Signature)
    };
    for field in source.source_shape().declared_fields() {
        let key = authority
            .nominal_field_key(field.field())
            .map_err(Error::Foundation)?;

        if PersistentFieldId::from_key(&key).ok() != Some(field.field()) {
            return Err(Error::Identity);
        }
        field_type(field.value_type(), authority)?;
    }
    if let NominalSourceShapeV1::Enum(shape) = source.source_shape() {
        for variant in shape.variants() {
            let key = authority
                .enum_variant_key(variant.variant())
                .map_err(Error::Foundation)?;

            if PersistentEnumVariantId::from_key(&key).ok() != Some(variant.variant())
                || key.source_owner() != Some(owner)
            {
                return Err(Error::Identity);
            }
            for field in variant.fields() {
                let key = authority
                    .enum_variant_field_key(field.field())
                    .map_err(Error::Foundation)?;

                if PersistentEnumVariantFieldId::from_key(&key).ok() != Some(field.field()) {
                    return Err(Error::Identity);
                }
                field_type(field.value_type(), authority)?;
            }
        }
    } else if let NominalSourceShapeV1::Object(shape) = source.source_shape() {
        let key = authority
            .object_value_key(shape.value())
            .map_err(Error::Foundation)?;

        if PersistentObjectValueId::from_source_object(&key).ok() != Some(shape.value()) {
            return Err(Error::Identity);
        }
    }
    source
        .source_shape()
        .validate_semantics(owner, source.kind(), source.type_parameters(), authority)
        .map_err(Error::Shape)
}
