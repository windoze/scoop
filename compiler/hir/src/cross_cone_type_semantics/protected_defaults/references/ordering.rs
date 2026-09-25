use std::cmp::Ordering;

use super::ProtectedDefaultReferenceV1;

pub(super) fn compare_keys<T: Ord>(
    left: &ProtectedDefaultReferenceV1<T>,
    right: &ProtectedDefaultReferenceV1<T>,
) -> Ordering {
    left.target()
        .cmp(right.target())
        .then_with(|| left.definition_origin().cmp(right.definition_origin()))
}
