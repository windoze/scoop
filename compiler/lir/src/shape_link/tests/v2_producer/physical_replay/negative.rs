use super::*;

pub(super) fn check<'a>(
    provider: &'a Provider,
    view: &ShapeLinkProviderV1<'a>,
    consumer: ConeIdentity,

    expected: &CanonicalExternalShapeLinkImportsV1,
) {
    let rows = expected.records();
    let replay = |rows: &[&dyn WireEncode]| {
        let decoded: DecodedCanonicalExternalShapeLinkImportsV1 =
            decode_canonical(&encode(&wire::Rows(rows)).unwrap()).unwrap();
        decoded.replay(
            consumer,
            std::slice::from_ref(view),
            &mut graph(provider, &[]),
        )
    };
    for row in rows {
        assert_eq!(replay(&[row]).unwrap().records().len(), 1);
        for index in 3..=5 {
            let other = rows
                .iter()
                .find(|value| value.subject() != row.subject())
                .unwrap();
            let symbol = other.expected_symbol();
            let definition = other.required_definition();
            let replacement: &dyn WireEncode = match index {
                3 => &symbol,
                4 => &definition,
                _ => other.contract(),
            };
            let changed = wire::ReplacedField {
                original: row,
                index,
                replacement,
            };
            assert!(
                replay(&[&changed]).is_err(),
                "accepted changed field {index} of {:?}",
                row.subject()
            );
        }
        let local = wire::ReplacedField {
            original: row,
            index: 1,
            replacement: &consumer,
        };
        assert!(replay(&[&local]).is_err());
    }
    let all = rows
        .iter()
        .map(|row| row as &dyn WireEncode)
        .collect::<Vec<_>>();
    let bytes = encode(&wire::Rows(&all)).unwrap();
    let decoded =
        || decode_canonical::<DecodedCanonicalExternalShapeLinkImportsV1>(&bytes).unwrap();
    assert!(
        matches!(decoded().replay(consumer, &[], &mut graph(provider, &[])),
        Err(ShapeLinkError::MissingProvider(id)) if id == provider.identity)
    );
    assert!(matches!(
        decoded().replay(
            provider.identity,
            std::slice::from_ref(view),
            &mut graph(provider, &[])
        ),
        Err(ShapeLinkError::LocalImport)
    ));
    assert!(matches!(
        decoded().replay(
            consumer,
            std::slice::from_ref(view),
            &mut PendingIdentityValidation::new().finish().unwrap()
        ),
        Err(ShapeLinkError::Identity(_))
    ));

    replay(&all).unwrap();

    replay(&all).unwrap();
}
