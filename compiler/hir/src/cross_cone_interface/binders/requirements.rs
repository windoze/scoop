use super::*;
use scoop_identity::MemberRequirementKey;

pub(super) fn encode<T: WireEncode>(
    values: &[MemberRequirementKey<T>],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    if !values.is_empty() {
        encoder.field(5)?;
        encoder.array(values.len() as u64)?;
        for value in values {
            value.encode(encoder)?;
        }
    }
    Ok(())
}

pub(super) fn resolve<R, E>(
    values: Vec<MemberRequirementKey<DecodedSignatureTypeKey>>,
    resolver: &mut R,
) -> Result<Vec<MemberRequirementKey>, TypeParameterBinderResolutionError<E>>
where
    R: SignatureTypeReferenceResolver<E>,
{
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let value = value
            .try_map(|ty| ty.clone().resolve(resolver))
            .map_err(TypeParameterBinderResolutionError::RequirementReference)?;
        if result.last().is_some_and(|previous| previous >= &value) {
            return Err(TypeParameterBinderResolutionError::RequirementOrder);
        }
        result.push(value);
    }
    Ok(result)
}
