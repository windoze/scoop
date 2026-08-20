use super::*;
use crate::{HirCLayoutContract, HirCLayoutValue};

fn policy() -> NominalCLayoutPolicyV1 {
    NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A16,
            packed: HirCLayoutValue::A2,
        },
    }
}

#[test]
fn source_policy_round_trips_with_exact_wire_and_resolved_identities() {
    let fixture = fixture();
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![struct_field(fixture.struct_first.id(), 0)],
            policy(),
            false,
        )
        .unwrap(),
    );
    let bytes = encode(&shape).unwrap();
    assert!(bytes.ends_with(&[
        0x02, 0xa2, 0x00, 0x02, 0x01, 0xa2, 0x01, 0xa1, 0x00, 0x06, 0x02, 0xa1, 0x00, 0x03, 0x03,
        0x00
    ]));
    let mut identities = authority(&fixture);

    assert_eq!(
        decode_shape(&shape).resolve(&mut identities).unwrap(),
        shape
    );
    assert_eq!(encode(&decode_shape(&shape)).unwrap(), bytes);
}

#[test]
fn source_policy_rejects_old_payload_unknown_policy_and_empty_c_layout() {
    let old = [0xa2, 0x00, 0x03, 0x01, 0x80];
    let error = decode_canonical::<DecodedNominalSourceShapeV1>(&old).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 4,
            actual: 2
        }
    ));
    let unknown = [
        0xa4, 0x00, 0x03, 0x01, 0x80, 0x02, 0xa1, 0x00, 0x03, 0x03, 0x00,
    ];
    let error = decode_canonical::<DecodedNominalSourceShapeV1>(&unknown).unwrap_err();
    assert!(matches!(error.kind(), WireErrorKind::UnknownTag { tag: 3 }));
    assert_eq!(
        StructSourceShapeV1::try_new(vec![], policy(), false),
        Err(NominalSourceShapeBuildError::EmptyCLayout)
    );
    let decoded = DecodedNominalSourceShapeV1::Struct {
        fields: vec![],
        c_layout_policy: policy(),
        interior_mutable: false,
    };
    let decoded = decode_shape(&decoded);
    assert!(matches!(
        decoded.resolve(&mut authority(&fixture())),
        Err(NominalSourceShapeResolutionError::EmptyCLayout)
    ));
}
