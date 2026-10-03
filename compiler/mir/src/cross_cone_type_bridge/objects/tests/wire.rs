use super::*;

#[test]
fn object_and_initialization_tables_round_trip_complete_products() {
    let mut fixture = Fixture::new();
    let objects =
        CanonicalMirObjectValuesV1::try_new(vec![fixture.object(0), fixture.object(1)]).unwrap();
    let decoded: DecodedCanonicalMirObjectValuesV1 =
        decode_canonical(&encode(&objects).unwrap()).unwrap();
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.types, &fixture.callables)
            .unwrap(),
        objects
    );
    let edge = fixture
        .use_for(
            1,
            MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
        )
        .unwrap();
    let uses = CanonicalMirExternalInitializationUsesV1::try_new(vec![edge]).unwrap();
    let decoded: DecodedCanonicalMirExternalInitializationUsesV1 =
        decode_canonical(&encode(&uses).unwrap()).unwrap();
    assert_eq!(
        decoded
            .validate(ConeIdentity::SINGLE_FILE, &mut fixture.graph)
            .unwrap(),
        uses
    );
}

#[test]
fn decoded_tables_reject_reordered_records() {
    let mut fixture = Fixture::new();
    let mut records = vec![fixture.object(0), fixture.object(1)];
    records.sort_by_key(|record| std::cmp::Reverse(record.value()));
    let decoded: DecodedCanonicalMirObjectValuesV1 =
        decode_canonical(&encode(&ObjectSequence(&records)).unwrap()).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.types, &fixture.callables),
        Err(MirObjectBridgeError::NonCanonicalObjectOrder { .. })
    ));
}

#[test]
fn read_plan_is_closed_and_rejects_unknown_tag_before_identity_resolution() {
    let fixture = Fixture::new();
    let mut bytes = encode(&fixture.object(0)).unwrap();
    let offset = bytes.len() - 38;
    assert_eq!(&bytes[offset..offset + 4], &[0xa2, 0, 1, 1]);
    bytes[offset + 2] = 2;
    assert!(decode_canonical::<DecodedParamFreeMirObjectValueV1>(&bytes).is_err());
}

struct ObjectSequence<'a>(&'a [ParamFreeMirObjectValueV1]);
impl WireEncode for ObjectSequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}

#[test]
fn initialization_reader_rejects_noncanonical_order() {
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
        decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(ConeIdentity::SINGLE_FILE, &mut fixture.graph),
        Err(MirObjectBridgeError::NonCanonicalInitializationUseOrder { .. })
    ));
}
struct UseSequence<'a>(&'a [SelectedExternalInitializationUseV1]);
impl WireEncode for UseSequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}
