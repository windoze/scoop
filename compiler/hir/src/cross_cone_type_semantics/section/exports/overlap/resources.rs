use super::*;

pub(super) fn signature(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,

    path: &WirePath,
) -> Result<bool, WireError> {
    crate::compare_default_signature_reference_targets(left, right, path)
        .map(|ordering| ordering.is_eq())
}
pub(super) fn signatures(
    left: &[SignatureTypeKey],
    right: &[SignatureTypeKey],

    path: &WirePath,
) -> Result<bool, WireError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        if !signature(left, right, &path.clone().index(index as u64))? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn optional_signature(
    left: Option<&SignatureTypeKey>,
    right: Option<&SignatureTypeKey>,

    path: &WirePath,
) -> Result<bool, WireError> {
    match (left, right) {
        (None, None) => Ok(true),
        (Some(left), Some(right)) => signature(left, right, path),
        _ => Ok(false),
    }
}
pub(super) fn binders(
    left: &CanonicalBinderListV1,
    right: &CanonicalBinderListV1,

    path: &WirePath,
) -> Result<bool, WireError> {
    if left.len_u32() != right.len_u32() {
        return Ok(false);
    }
    for (index, (left, right)) in left.binders().iter().zip(right.binders()).enumerate() {
        let at = path.clone().index(index as u64);

        if left.name() != right.name() {
            return Ok(false);
        }
        match (left.bounds(), right.bounds()) {
            (TypeParameterBoundsV1::Nominal(left), TypeParameterBoundsV1::Nominal(right)) => {
                if !optional_signature(left.class(), right.class(), &at)?
                    || !signatures(left.interfaces().values(), right.interfaces().values(), &at)?
                {
                    return Ok(false);
                }
            }
            (TypeParameterBoundsV1::Unconstrained, TypeParameterBoundsV1::Unconstrained)
            | (TypeParameterBoundsV1::Value, TypeParameterBoundsV1::Value)
            | (TypeParameterBoundsV1::Ref, TypeParameterBoundsV1::Ref) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

pub(super) fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
