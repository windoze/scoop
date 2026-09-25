use super::*;

#[test]
fn dispatch_canonical_wire_round_trip_preserves_complete_schema() {
    let mut fixture = Fixture::new();
    let table = fixture.table();
    let bytes = encode(&table).unwrap();
    let decoded: DecodedCanonicalMirDispatchSchemasV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.types, &fixture.callables)
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
    assert!(decode_canonical::<MirDispatchReceiverAdaptationV1>(&[0xa1, 0, 3]).is_err());
    assert!(decode_canonical::<MirDispatchReceiverAdaptationV1>(&[0xa2, 0, 1, 1, 0]).is_err());
}

#[test]
fn reader_rejects_unsorted_schema_without_repairing_it() {
    let mut fixture = Fixture::new();
    let mut table = fixture.table();
    table.records.reverse();
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&encode(&table).unwrap()).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.types, &fixture.callables),
        Err(MirDispatchSchemaError::NonCanonicalOwnerOrder { .. })
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
        decode_canonical(&encode(&table).unwrap()).unwrap();
    assert!(
        matches!(decoded.validate(&mut fixture.graph, &fixture.types, &fixture.callables), Err(MirDispatchSchemaError::MissingCallable { target: missing }) if missing == target)
    );
}
