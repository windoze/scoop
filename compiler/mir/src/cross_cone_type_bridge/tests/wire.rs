use super::*;

#[test]
fn source_and_finite_helpers_round_trip_through_the_canonical_table() {
    let mut fixture = Fixture::new();
    let records = vec![
        fixture.slot_export(),
        fixture.empty_export(),
        fixture.step_export(),
        fixture.boxed_export(),
    ];
    let table = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
    let encoded = encode(&table).unwrap();
    let decoded: DecodedCanonicalParamFreeMirTypeExportsV1 = decode_canonical(&encoded).unwrap();
    assert_eq!(encode(&decoded).unwrap(), encoded);
    let restored = decoded
        .validate(&mut fixture.graph, &fixture.foundation)
        .unwrap();
    assert_eq!(restored, table);
    assert!(restored.get(fixture.payload.id()).is_some());
    assert!(
        restored
            .records()
            .windows(2)
            .all(|pair| pair[0].exact() < pair[1].exact())
    );
}

#[test]
fn decoded_tables_reject_duplicates_and_noncanonical_record_order() {
    let mut fixture = Fixture::new();
    let first = fixture.empty_export();
    assert!(matches!(
        CanonicalParamFreeMirTypeExportsV1::try_new(vec![first.clone(), first.clone()]),
        Err(MirTypeBridgeError::DuplicateType { .. })
    ));
    let second = fixture.boxed_export();
    for mut records in [
        vec![first.clone(), first],
        vec![second, fixture.empty_export()],
    ] {
        records.sort_unstable_by_key(|record| std::cmp::Reverse(record.exact()));
        let mut bytes = vec![0x82];
        for record in records {
            bytes.extend(encode(&record).unwrap());
        }
        // The payload is valid CBOR but the semantic table order is not.
        let decoded: DecodedCanonicalParamFreeMirTypeExportsV1 = decode_canonical(&bytes).unwrap();
        assert!(matches!(
            decoded.validate(&mut fixture.graph, &fixture.foundation),
            Err(MirTypeBridgeError::NonCanonicalTypeOrder { index: 1 })
        ));
    }
}

#[test]
fn type_export_wire_is_closed_and_validates_rebound_exact_identity() {
    let mut fixture = Fixture::new();
    let export = fixture.empty_export();
    let mut bytes = encode(&export).unwrap();
    assert_eq!(bytes[0], 0xa5, "all five required fields are present");
    let start = bytes
        .windows(32)
        .position(|bytes| bytes == export.exact().as_array())
        .unwrap();
    bytes[start..start + 32].copy_from_slice(exact(fixture.other.id()).id().as_array());
    let decoded: DecodedParamFreeMirTypeExportV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.foundation),
        Err(MirTypeBridgeError::ExactOriginMismatch { .. })
    ));
    for bytes in [vec![0xa4], vec![0xa6], vec![0xa5, 0x02]] {
        assert!(decode_canonical::<DecodedParamFreeMirTypeExportV1>(&bytes).is_err());
    }
}

#[test]
fn source_c_layout_policy_and_scalar_kind_have_separate_fixed_wire() {
    assert_eq!(
        encode(&MirParamFreeIntrinsicV1::Unit).unwrap(),
        vec![0xa1, 0, 1]
    );
    for kind in crate::IntegerKind::ALL {
        let value = MirParamFreeIntrinsicV1::Integer(kind);
        assert_eq!(
            decode_canonical::<MirParamFreeIntrinsicV1>(&encode(&value).unwrap()).unwrap(),
            value
        );
    }
    let c_layout = MirTypeCLayoutPolicyV1::CLayout(crate::MirCLayoutContract {
        aligned: crate::MirCLayoutValue::A16,
        packed: crate::MirCLayoutValue::A1,
    });
    assert_eq!(
        decode_canonical::<MirTypeCLayoutPolicyV1>(&encode(&c_layout).unwrap()).unwrap(),
        c_layout
    );
    assert!(decode_canonical::<DecodedMirTypeRepresentationV1>(&[0xa1, 0, 99]).is_err());
}
