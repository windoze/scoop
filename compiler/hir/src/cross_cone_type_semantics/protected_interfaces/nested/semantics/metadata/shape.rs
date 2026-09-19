use super::*;
use scoop_identity::{
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentObjectValueId,
};

pub(super) fn validate<A: NestedNominalSemanticAuthority<E>, E>(
    owner: SourceNominalId,
    source: &ProtectedNestedSourceInterfaceV1,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    let scope = source.type_parameters().signature_scope(None);
    let field_type = |ty: &SignatureTypeKey, authority: &mut A, meter: &mut BudgetMeter| {
        scope
            .validate_signature_semantics_metered(ty, authority, meter, &WirePath::root())
            .map_err(Error::Signature)
    };
    match source.source_shape() {
        NominalSourceShapeV1::Struct(shape) => {
            for field in shape.fields() {
                let key = authority
                    .struct_field_key(field.field())
                    .map_err(Error::Foundation)?;
                charge_key(&key, meter)?;
                if PersistentFieldId::from_key(&key).ok() != Some(field.field())
                    || key.source_owner() != Some(owner)
                {
                    return Err(Error::Identity);
                }
                field_type(field.value_type(), authority, meter)?;
            }
        }
        NominalSourceShapeV1::Enum(shape) => {
            for variant in shape.variants() {
                let key = authority
                    .enum_variant_key(variant.variant())
                    .map_err(Error::Foundation)?;
                charge_key(&key, meter)?;
                if PersistentEnumVariantId::from_key(&key).ok() != Some(variant.variant())
                    || key.source_owner() != Some(owner)
                {
                    return Err(Error::Identity);
                }
                for field in variant.fields() {
                    let key = authority
                        .enum_variant_field_key(field.field())
                        .map_err(Error::Foundation)?;
                    charge_key(&key, meter)?;
                    if PersistentEnumVariantFieldId::from_key(&key).ok() != Some(field.field()) {
                        return Err(Error::Identity);
                    }
                    field_type(field.value_type(), authority, meter)?;
                }
            }
        }
        NominalSourceShapeV1::Object(shape) => {
            let key = authority
                .object_value_key(shape.value())
                .map_err(Error::Foundation)?;
            charge_key(&key, meter)?;
            if PersistentObjectValueId::from_source_object(&key).ok() != Some(shape.value()) {
                return Err(Error::Identity);
            }
        }
        NominalSourceShapeV1::Class | NominalSourceShapeV1::Interface => {}
    }
    // Preserve the existing source field selector and object owner semantics;
    // all recursive type work and key hashing was metered above.
    source
        .source_shape()
        .validate_semantics(owner, source.kind(), source.type_parameters(), authority)
        .map_err(Error::Shape)
}
fn charge_key<K: WireEncode, E>(
    key: &K,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    meter
        .charge_sha256(
            scoop_wire::encoded_length(key).map_err(Error::Encoding)?,
            &WirePath::root(),
        )
        .map_err(Error::Resource)
}
