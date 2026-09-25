use super::*;

pub(super) fn check(
    expected: &lir::CanonicalExactLayoutExportsV1,
    foundation: &lir::OdrFreeLirFoundation,
    replay: impl Fn(
        &lir::CanonicalExactLayoutExportsV1,
    ) -> Result<lir::CrossConeLirBridgeSectionV1, Error>,
) {
    let original = expected
        .records()
        .iter()
        .find_map(|record| match record.kind() {
            lir::ExactLayoutBodyKindV1::Value(value)
                if matches!(
                    value.representation().kind(),
                    lir::ExactRepresentationKindV1::Scalar(
                        lir::ScalarRepresentationKindV1::Boolean
                    )
                ) =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap();
    let exact = original.identity().exact();
    let changed = lir::ExactValueLayoutV1::scalar(
        original.identity().clone(),
        lir::ScalarRepresentationKindV1::Integer(lir::IntegerKind::SIGNED_64),
        foundation,
    )
    .unwrap();
    let mut records = expected
        .records()
        .iter()
        .filter(|record| record.identity() != original.identity())
        .cloned()
        .collect::<Vec<_>>();
    records.push(changed.into());
    let layouts =
        lir::CanonicalExactLayoutExportsV1::try_new(expected.target(), foundation, records)
            .unwrap();
    assert!(
        matches!(replay(&layouts), Err(Error::LayoutSignature { exact: actual, .. }) if actual == exact)
    );
}
