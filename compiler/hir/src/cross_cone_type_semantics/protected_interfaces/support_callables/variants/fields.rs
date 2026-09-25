use super::*;
use crate::{EnumSourceVariantStyleV1, EnumSourceVariantV1};
use scoop_identity::{EnumVariantFieldSelector, PersistentEnumVariantFieldId};

pub(super) fn validate<A: NominalSupportCallableSemanticAuthority<E>, E>(
    variant: PersistentEnumVariantId,
    payload: &NominalSourceCallablePayloadV1,
    shape: &EnumSourceVariantV1,
    authority: &A,
) -> Result<(), NominalSupportCallableSemanticError<E>> {
    use NominalSupportCallableSemanticError as Error;
    use NominalSupportVariantError as VariantError;
    let fail = || Error::Variant(VariantError::Parameters);
    let parameters = payload.parameters().parameters();
    if shape.variant() != variant || parameters.len() != shape.fields().len() {
        return Err(fail());
    }

    for (index, (parameter, field)) in parameters.iter().zip(shape.fields()).enumerate() {
        let key = authority
            .source_enum_variant_field_key(field.field())
            .map_err(Error::Foundation)?;

        if PersistentEnumVariantFieldId::from_key(key).ok() != Some(field.field())
            || key.variant() != variant
            || parameter.value_type() != field.value_type()
        {
            return Err(fail());
        }
        let matches = match (shape.style(), key.selector()) {
            (
                EnumSourceVariantStyleV1::Positional,
                EnumVariantFieldSelector::Positional { declaration_index },
            ) => *declaration_index as usize == index,
            (
                EnumSourceVariantStyleV1::Named | EnumSourceVariantStyleV1::Constructor,
                EnumVariantFieldSelector::Named(name),
            ) => name == parameter.name(),
            _ => false,
        };
        if !matches {
            return Err(fail());
        }
    }
    Ok(())
}
