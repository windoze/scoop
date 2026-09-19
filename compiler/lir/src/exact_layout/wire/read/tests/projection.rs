use super::*;

#[test]
fn semantic_projection_replays_all_layout_families_without_definition_fields() {
    for expected in [
        unit().into(),
        fixtures::aggregate(),
        fixtures::tuple(),
        fixtures::enumeration(false),
        fixtures::enumeration(true),
        fixtures::bytes(),
        fixtures::array(true),
        fixtures::array(false),
    ] {
        let bytes = encode(&expected.semantic_projection()).unwrap();
        assert_eq!(bytes[0], 0xa6);
        let decoded: DecodedExactLayoutSemanticProjectionV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        decoded.validate_against(&expected, &mut meter()).unwrap();
        // The complete legacy record is exactly the same six-field prefix
        // followed by the original field 7, without renumbering any field.
        let mut complete = bytes;
        complete[0] = 0xa7;
        complete.push(7);
        complete.extend(encode(&expected.identity().definition()).unwrap());
        assert_eq!(encode(&expected).unwrap(), complete);
    }
}

#[test]
fn semantic_projection_rejects_changed_typed_geometry_and_full_record_payload() {
    let expected = fixtures::aggregate();
    let bytes = encode(&expected.semantic_projection()).unwrap();
    let mut decoded: DecodedExactLayoutSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let RawBody::Value {
        representation: RawValue::Struct { fields, .. },
        ..
    } = &mut decoded.body
    else {
        panic!("struct")
    };
    fields[1].alignment = 1;
    assert!(decoded.validate_against(&expected, &mut meter()).is_err());
    assert!(
        decode_canonical::<DecodedExactLayoutSemanticProjectionV1>(
            &encode(&expected).unwrap(),
            DecodeLimits::default()
        )
        .is_err()
    );
    let decoded: DecodedExactLayoutSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(
        decoded
            .validate_against(
                &expected,
                &mut BudgetMeter::new(DecodeLimits {
                    validation_work_units: 5,
                    ..DecodeLimits::default()
                })
            )
            .is_err()
    );
}
