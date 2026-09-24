use super::*;
use scoop_wire::BudgetMeter;

#[test]
fn declared_interior_mutability_has_distinct_canonical_bytes_and_survives_resolution() {
    for interior_mutable in [false, true] {
        let shape = NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![],
                NominalCLayoutPolicyV1::Ordinary,
                interior_mutable,
            )
            .unwrap(),
        );
        let bytes = encode(&shape).unwrap();
        assert_eq!(
            bytes,
            [
                0xa4,
                0x00,
                0x03,
                0x01,
                0x80,
                0x02,
                0xa1,
                0x00,
                0x01,
                0x03,
                u8::from(interior_mutable)
            ]
        );
        let decoded: DecodedNominalSourceShapeV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let restored = decoded
            .resolve_metered(
                &mut authority(&fixture()),
                &mut BudgetMeter::new(DecodeLimits::default()),
            )
            .unwrap();
        assert_eq!(restored, shape);
        let NominalSourceShapeV1::Struct(restored) = restored else {
            panic!("decoded a struct");
        };
        assert_eq!(restored.interior_mutable(), interior_mutable);
    }
}

#[test]
fn struct_reader_rejects_missing_policy_and_non_boolean_policy_values() {
    let retired = [0xa3, 0x00, 0x03, 0x01, 0x80, 0x02, 0xa1, 0x00, 0x01];
    let error = decode_canonical::<DecodedNominalSourceShapeV1>(&retired, DecodeLimits::default())
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 4,
            actual: 3
        }
    ));
    for value in [2, 23] {
        let malformed = [
            0xa4, 0x00, 0x03, 0x01, 0x80, 0x02, 0xa1, 0x00, 0x01, 0x03, value,
        ];
        let error =
            decode_canonical::<DecodedNominalSourceShapeV1>(&malformed, DecodeLimits::default())
                .unwrap_err();
        assert!(
            matches!(error.kind(), WireErrorKind::UnknownTag { tag } if *tag == u64::from(value))
        );
        assert_eq!(error.path(), &scoop_wire::WirePath::root().field(3));
    }
}
