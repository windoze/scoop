use super::*;

#[test]
fn complete_shape_products_round_trip_for_value_and_reference() {
    for mut family in [Family::value(false), Family::reference(true)] {
        let table = family.table();
        let bytes = encode(&table).unwrap();
        let decoded: DecodedCanonicalMirShapeSupportsV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(
            decoded
                .validate(
                    ConeIdentity::SINGLE_FILE,
                    &mut family.fixture.graph,
                    &family.types
                )
                .unwrap(),
            table
        );
    }
}

#[test]
fn reader_rejects_duplicate_sources_and_unknown_box_availability() {
    let mut family = Family::value(false);
    let record = family.build().unwrap();
    let bytes = encode(&Records(&[record.clone(), record.clone()])).unwrap();
    let decoded: DecodedCanonicalMirShapeSupportsV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(
            ConeIdentity::SINGLE_FILE,
            &mut family.fixture.graph,
            &family.types
        ),
        Err(MirShapeSupportError::NonCanonicalSourceOrder { .. })
    ));
    let mut bytes = encode(&record).unwrap();
    let offset = bytes
        .windows(4)
        .position(|window| window == [3, 0xa2, 0, 1])
        .unwrap();
    bytes[offset + 3] = 3;
    assert!(decode_canonical::<DecodedParamFreeMirShapeSupportV1>(&bytes).is_err());
}

struct Records<'a>(&'a [ParamFreeMirShapeSupportV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}
