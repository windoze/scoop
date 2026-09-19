use super::*;

#[test]
fn dispatch_canonical_wire_round_trip_preserves_complete_schema() {
    let mut fixture = Fixture::new();
    let table = fixture.table();
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .validate(
                &mut fixture.graph,
                &fixture.types,
                &fixture.callables,
                &mut meter()
            )
            .unwrap(),
        table
    );
    assert_eq!(
        encode(&MirDispatchReceiverAdaptationV1::Identity).unwrap(),
        vec![0xa1, 0, 1]
    );
    assert_eq!(
        encode(&MirDispatchReceiverAdaptationV1::ReferenceDispatch).unwrap(),
        vec![0xa1, 0, 2]
    );
    assert!(
        decode_canonical::<MirDispatchReceiverAdaptationV1>(&[0xa1, 0, 3], DecodeLimits::default())
            .is_err()
    );
    assert!(
        decode_canonical::<MirDispatchReceiverAdaptationV1>(
            &[0xa2, 0, 1, 1, 0],
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn reader_rejects_unsorted_schema_without_repairing_it() {
    let mut fixture = Fixture::new();
    let mut table = fixture.table();
    table.records.reverse();
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(
            &mut fixture.graph,
            &fixture.types,
            &fixture.callables,
            &mut meter()
        ),
        Err(MirDispatchSchemaError::NonCanonicalOwnerOrder { .. })
    ));
}

#[test]
fn schema_validation_and_path_replay_share_the_callers_budget() {
    let fixture = Fixture::new();
    let records = fixture.table().records;
    let limits = DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(
            fixture.authority(),
            records.clone(),
            &mut BudgetMeter::new(limits)
        ),
        Err(MirDispatchSchemaError::Resource(_))
    ));
    let limits = DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(
            fixture.authority(),
            records,
            &mut BudgetMeter::new(limits)
        ),
        Err(MirDispatchSchemaError::Resource(_))
    ));
}

#[test]
fn repeated_path_validation_cannot_reset_work_or_recursion_limits() {
    let fixture = Fixture::new();
    let mut first = meter();
    fixture
        .authority()
        .canonical_receiver_path(fixture.exact(DERIVED), fixture.exact(ROOT), &mut first)
        .unwrap();
    let mut limited = BudgetMeter::new(DecodeLimits {
        validation_work_units: first.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    fixture
        .authority()
        .canonical_receiver_path(fixture.exact(DERIVED), fixture.exact(ROOT), &mut limited)
        .unwrap();
    assert!(matches!(
        fixture.authority().canonical_receiver_path(
            fixture.exact(DERIVED),
            fixture.exact(ROOT),
            &mut limited
        ),
        Err(MirDispatchSchemaError::Resource(_))
    ));
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(
            fixture.authority(),
            fixture.table().records,
            &mut BudgetMeter::new(DecodeLimits {
                semantic_recursion: 1,
                ..DecodeLimits::default()
            })
        ),
        Err(MirDispatchSchemaError::Resource(_))
    ));
}

#[test]
fn reader_requires_a_closed_callable_target_table() {
    let mut fixture = Fixture::new();
    let table = fixture.table();
    let target = fixture.target(5);
    fixture.callables = CanonicalMirCallableBindingsV1::try_new(
        fixture
            .callables
            .entries()
            .iter()
            .filter(|binding| binding.implementation() != target)
            .cloned()
            .collect(),
    )
    .unwrap();
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    assert!(
        matches!(decoded.validate(&mut fixture.graph, &fixture.types, &fixture.callables, &mut meter()), Err(MirDispatchSchemaError::MissingCallable { target: missing }) if missing == target)
    );
}
