use super::*;

#[test]
fn object_and_initialization_tables_round_trip_complete_products() {
    let mut fixture = Fixture::new();
    let objects = CanonicalMirObjectValuesV1::try_new(
        vec![fixture.object(0), fixture.object(1)],
        &mut meter(),
    )
    .unwrap();
    let decoded: DecodedCanonicalMirObjectValuesV1 =
        decode_canonical(&encode(&objects).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded
            .validate(
                &mut fixture.graph,
                &fixture.types,
                &fixture.callables,
                &mut meter()
            )
            .unwrap(),
        objects
    );
    let edge = fixture
        .use_for(
            1,
            MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
        )
        .unwrap();
    let uses = CanonicalMirExternalInitializationUsesV1::try_new(vec![edge], &mut meter()).unwrap();
    let decoded: DecodedCanonicalMirExternalInitializationUsesV1 =
        decode_canonical(&encode(&uses).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded
            .validate(ConeIdentity::SINGLE_FILE, &mut fixture.graph, &mut meter())
            .unwrap(),
        uses
    );
}

#[test]
fn decoded_tables_reject_reordered_records_and_share_budget() {
    let mut fixture = Fixture::new();
    let mut records = vec![fixture.object(0), fixture.object(1)];
    records.sort_by_key(|record| std::cmp::Reverse(record.value()));
    let decoded: DecodedCanonicalMirObjectValuesV1 = decode_canonical(
        &encode(&ObjectSequence(&records)).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.validate(
            &mut fixture.graph,
            &fixture.types,
            &fixture.callables,
            &mut meter()
        ),
        Err(MirObjectBridgeError::NonCanonicalObjectOrder { .. })
    ));
    let bytes =
        encode(&CanonicalMirObjectValuesV1::try_new(records, &mut meter()).unwrap()).unwrap();
    let decoded: DecodedCanonicalMirObjectValuesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(
            &mut fixture.graph,
            &fixture.types,
            &fixture.callables,
            &mut BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(MirObjectBridgeError::Resource(_))
    ));
}

#[test]
fn read_plan_is_closed_and_rejects_unknown_tag_before_identity_resolution() {
    let fixture = Fixture::new();
    let mut bytes = encode(&fixture.object(0)).unwrap();
    let offset = bytes.len() - 38;
    assert_eq!(&bytes[offset..offset + 4], &[0xa2, 0, 1, 1]);
    bytes[offset + 2] = 2;
    assert!(
        decode_canonical::<DecodedParamFreeMirObjectValueV1>(&bytes, DecodeLimits::default())
            .is_err()
    );
}

struct ObjectSequence<'a>(&'a [ParamFreeMirObjectValueV1]);
impl WireEncode for ObjectSequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}

#[test]
fn initialization_reader_rejects_noncanonical_order_and_checks_work_before_resolution() {
    let mut fixture = Fixture::new();
    let mut records = vec![
        fixture
            .use_for(
                1,
                MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
            )
            .unwrap(),
        fixture
            .use_for(
                1,
                MirExternalInitializationCauseV1::InitializationSupport(fixture.units[1].id()),
            )
            .unwrap(),
    ];
    records.sort_by_key(|record| std::cmp::Reverse(*record));
    let bytes = encode(&UseSequence(&records)).unwrap();
    let decoded: DecodedCanonicalMirExternalInitializationUsesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(ConeIdentity::SINGLE_FILE, &mut fixture.graph, &mut meter()),
        Err(MirObjectBridgeError::NonCanonicalInitializationUseOrder { .. })
    ));
    let decoded: DecodedSelectedExternalInitializationUseV1 =
        decode_canonical(&encode(&records[0]).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(
            ConeIdentity::SINGLE_FILE,
            &mut fixture.graph,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(MirObjectBridgeError::Resource(_))
    ));
}
struct UseSequence<'a>(&'a [SelectedExternalInitializationUseV1]);
impl WireEncode for UseSequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}

#[test]
fn object_source_key_comparison_has_an_inclusive_shared_work_limit() {
    let fixture = Fixture::new();
    let reference = fixture.object(0);
    let build = |meter: &mut BudgetMeter| {
        ParamFreeMirObjectValueV1::try_new(
            fixture.authority(),
            reference.value(),
            reference.backing(),
            reference.unit(),
            reference.ensure(),
            reference.read(),
            meter,
        )
    };
    let mut measured = meter();
    build(&mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    assert_eq!(build(&mut exact).unwrap(), reference);
    assert!(matches!(
        build(&mut exact),
        Err(MirObjectBridgeError::Resource(_))
    ));
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: required - 1,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        build(&mut short),
        Err(MirObjectBridgeError::Resource(_))
    ));
}
