use super::*;
use crate::{HirCLayoutContract, HirCLayoutValue};
use scoop_wire::{BudgetMeter, WirePath};

fn policy() -> NominalCLayoutPolicyV1 {
    NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A16,
            packed: HirCLayoutValue::A2,
        },
    }
}

#[test]
fn source_policy_round_trips_with_exact_wire_and_shared_resolution_budget() {
    let fixture = fixture();
    let shape = NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(vec![struct_field(fixture.struct_first.id(), 0)], policy())
            .unwrap(),
    );
    let bytes = encode(&shape).unwrap();
    assert!(bytes.ends_with(&[
        0x02, 0xa2, 0x00, 0x02, 0x01, 0xa2, 0x01, 0xa1, 0x00, 0x06, 0x02, 0xa1, 0x00, 0x03
    ]));
    let mut identities = authority(&fixture);
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    assert_eq!(
        decode_shape(&shape)
            .resolve_metered(&mut identities, &mut meter)
            .unwrap(),
        shape
    );
    assert_eq!(encode(&decode_shape(&shape)).unwrap(), bytes);
    let limit = DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        decode_shape(&shape).resolve_metered(&mut identities, &mut BudgetMeter::new(limit)),
        Err(crate::MeteredInterfaceResolutionError::Resource(_))
    ));
    let mut depleted = BudgetMeter::new(DecodeLimits {
        decoded_nodes: 1,
        ..DecodeLimits::default()
    });
    depleted.charge_nodes(1, &WirePath::root()).unwrap();
    assert!(matches!(
        decode_shape(&shape).resolve_metered(&mut identities, &mut depleted),
        Err(crate::MeteredInterfaceResolutionError::Resource(_))
    ));
}

#[test]
fn source_policy_rejects_old_payload_unknown_policy_and_empty_c_layout() {
    let old = [0xa2, 0x00, 0x03, 0x01, 0x80];
    let error =
        decode_canonical::<DecodedNominalSourceShapeV1>(&old, DecodeLimits::default()).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 3,
            actual: 2
        }
    ));
    let unknown = [0xa3, 0x00, 0x03, 0x01, 0x80, 0x02, 0xa1, 0x00, 0x03];
    let error = decode_canonical::<DecodedNominalSourceShapeV1>(&unknown, DecodeLimits::default())
        .unwrap_err();
    assert!(matches!(error.kind(), WireErrorKind::UnknownTag { tag: 3 }));
    assert_eq!(
        StructSourceShapeV1::try_new(vec![], policy()),
        Err(NominalSourceShapeBuildError::EmptyCLayout)
    );
    let decoded = DecodedNominalSourceShapeV1::Struct {
        fields: vec![],
        c_layout_policy: policy(),
    };
    let decoded = decode_shape(&decoded);
    assert!(matches!(
        decoded.resolve(&mut authority(&fixture())),
        Err(NominalSourceShapeResolutionError::EmptyCLayout)
    ));
}
