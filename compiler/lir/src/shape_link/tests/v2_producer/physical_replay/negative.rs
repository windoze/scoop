use super::*;
use scoop_wire::WirePath;

pub(super) fn check<'a>(
    provider: &'a Provider,
    view: &ShapeLinkProviderV1<'a>,
    consumer: ConeIdentity,
    definitions: &StrongObjectSymbolSurfaceV1,
    support: &fixtures::UnitSupport<'a>,
    expected: &CanonicalExternalShapeLinkImportsV1<'a>,
) {
    let rows = expected.records();
    let replay = |rows: &[&dyn WireEncode], meter: &mut BudgetMeter| {
        let decoded: DecodedCanonicalExternalShapeLinkImportsV1 =
            decode_canonical(&encode(&wire::Rows(rows)).unwrap(), DecodeLimits::default()).unwrap();
        decoded.replay(
            consumer,
            definitions,
            std::slice::from_ref(view),
            support,
            &mut graph(provider, &[]),
            meter,
        )
    };
    for row in rows {
        assert_eq!(replay(&[row], &mut meter()).unwrap().records().len(), 1);
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
                replay(&[&changed], &mut meter()).is_err(),
                "accepted changed field {index} of {:?}",
                row.subject()
            );
        }
        let local = wire::ReplacedField {
            original: row,
            index: 1,
            replacement: &consumer,
        };
        assert!(replay(&[&local], &mut meter()).is_err());
    }
    let all = rows
        .iter()
        .map(|row| row as &dyn WireEncode)
        .collect::<Vec<_>>();
    let bytes = encode(&wire::Rows(&all)).unwrap();
    let decoded = || {
        decode_canonical::<DecodedCanonicalExternalShapeLinkImportsV1>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap()
    };
    assert!(
        matches!(decoded().replay(consumer, definitions, &[], support, &mut graph(provider, &[]), &mut meter()),
        Err(ShapeLinkError::MissingProvider(id)) if id == provider.identity)
    );
    assert!(matches!(
        decoded().replay(
            consumer,
            definitions,
            std::slice::from_ref(view),
            &NoShapeLinkSupportV1,
            &mut graph(provider, &[]),
            &mut meter()
        ),
        Err(ShapeLinkError::SupportRelation(_))
    ));
    assert!(matches!(
        decoded().replay(
            provider.identity,
            definitions,
            std::slice::from_ref(view),
            support,
            &mut graph(provider, &[]),
            &mut meter()
        ),
        Err(ShapeLinkError::LocalImport)
    ));
    assert!(matches!(
        decoded().replay(
            consumer,
            definitions,
            std::slice::from_ref(view),
            support,
            &mut PendingIdentityValidation::new().finish().unwrap(),
            &mut meter()
        ),
        Err(ShapeLinkError::Identity(_))
    ));

    let mut measured = meter();
    replay(&all, &mut measured).unwrap();
    let work = measured.usage().validation_work_units;
    let limits = DecodeLimits {
        validation_work_units: work,
        ..DecodeLimits::default()
    };
    replay(&all, &mut BudgetMeter::new(limits)).unwrap();
    let mut cumulative = BudgetMeter::new(limits);
    cumulative.charge_work(1, &WirePath::root()).unwrap();
    assert!(replay(&all, &mut cumulative).is_err());
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(replay(&all, &mut BudgetMeter::new(limits)).is_err());
    }
}
