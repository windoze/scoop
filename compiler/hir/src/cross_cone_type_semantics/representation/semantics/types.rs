use super::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::WireError;

pub(super) fn fields<'a, 'b, I: PartialEq>(
    left: impl ExactSizeIterator<Item = (I, &'a SignatureTypeKey)>,
    right: impl ExactSizeIterator<Item = (I, &'b SignatureTypeKey)>,

    path: &WirePath,
) -> Result<bool, WireError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (index, ((left_id, left), (right_id, right))) in left.zip(right).enumerate() {
        let at = path.clone().index(index as u64);

        if left_id != right_id
            || !crate::compare_default_signature_reference_targets(left, right, &at)
                .map(|ordering| ordering.is_eq())?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
