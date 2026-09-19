use super::*;

#[test]
fn complete_shape_products_round_trip_for_value_and_reference() {
    for mut family in [Family::value(false), Family::reference(true)] {
        let table = family.table();
        let bytes = encode(&table).unwrap();
        let decoded: DecodedCanonicalMirShapeSupportsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded
                .validate(
                    ConeIdentity::SINGLE_FILE,
                    &mut family.fixture.graph,
                    &family.types,
                    &mut meter()
                )
                .unwrap(),
            table
        );
    }
}

#[test]
fn reader_rejects_duplicate_sources_and_unknown_box_availability() {
    let mut family = Family::value(false);
    let record = family.build(&mut meter()).unwrap();
    let bytes = encode(&Records(&[record.clone(), record.clone()])).unwrap();
    let decoded: DecodedCanonicalMirShapeSupportsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.validate(
            ConeIdentity::SINGLE_FILE,
            &mut family.fixture.graph,
            &family.types,
            &mut meter()
        ),
        Err(MirShapeSupportError::NonCanonicalSourceOrder { .. })
    ));
    let mut bytes = encode(&record).unwrap();
    let offset = bytes
        .windows(4)
        .position(|window| window == [3, 0xa2, 0, 1])
        .unwrap();
    bytes[offset + 3] = 3;
    assert!(
        decode_canonical::<DecodedParamFreeMirShapeSupportV1>(&bytes, DecodeLimits::default())
            .is_err()
    );
}

#[test]
fn reader_charges_allocation_and_validation_before_identity_lookups() {
    let mut family = Family::value(false);
    let bytes = encode(&family.table()).unwrap();
    for limits in [
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        let decoded: DecodedCanonicalMirShapeSupportsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.validate(
                ConeIdentity::SINGLE_FILE,
                &mut family.fixture.graph,
                &family.types,
                &mut BudgetMeter::new(limits)
            ),
            Err(MirShapeSupportError::Resource(_))
        ));
    }
}

#[test]
fn family_semantic_validation_has_an_inclusive_work_budget() {
    let family = Family::reference(true);
    let mut measured = meter();
    family.build(&mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    family
        .build(&mut BudgetMeter::new(DecodeLimits {
            validation_work_units: required,
            ..DecodeLimits::default()
        }))
        .unwrap();
    assert!(matches!(
        family.build(&mut BudgetMeter::new(DecodeLimits {
            validation_work_units: required - 1,
            ..DecodeLimits::default()
        })),
        Err(MirShapeSupportError::Resource(_))
    ));
}

struct Records<'a>(&'a [ParamFreeMirShapeSupportV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}
