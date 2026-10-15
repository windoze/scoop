use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::link_object::LinkIdentityClosureSectionV1;

use crate::link_object::strong_relocation_closure::tests::verified_member_without_relocations;
use crate::link_object::symbol_verification::tests::fixture_named;
use crate::link_object::undefined_requirements::tests::empty_final_requirements_for_strong;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalScoopLirObjectUnitSetV1,
    verify_current_cone_strong_relocation_closure_v1,
};

#[test]
fn closure_wire_round_trips_saved_fields() {
    let bytes = encode(&closure()).unwrap();
    assert_eq!(bytes[0], 0xa3);
    let decoded = decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn closure_reader_rejects_old_extended_and_unknown_sum_shapes() {
    for bytes in [vec![0xa2], vec![0xa4], vec![0xa8]] {
        assert!(decode_canonical::<DecodedLinkIdentityClosureSectionV1>(&bytes).is_err());
    }

    let expected = closure();
    let mut materialization = encode(&expected.materializations[0]).unwrap();
    assert_eq!(&materialization[..3], &[0xa3, 0x00, 0x01]);
    materialization[2] = 3;
    assert!(decode_canonical::<DecodedLinkObjectMaterializationV1>(&materialization).is_err());
}

fn closure() -> LinkIdentityClosureSectionV1 {
    let fixture = fixture_named("closureWireProjection");
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_without_relocations(&fixture),
    ])
    .unwrap();
    let member = &strong.members()[0];
    let definition = &member.definitions().definitions()[0];
    let member_id = member.member();
    let definition_id = definition.definition();
    let defined_symbols =
        CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong).unwrap();
    let undefined_symbols = empty_final_requirements_for_strong(strong);

    LinkIdentityClosureSectionV1 {
        materializations: vec![super::super::LinkObjectMaterializationV1::ScoopLir {
            member: member_id,
            units: CanonicalScoopLirObjectUnitSetV1::new(vec![definition_id]).unwrap(),
        }],
        defined_symbols,
        undefined_symbols,
    }
}

pub(crate) fn encoded_link_identity_closure_for_test() -> Vec<u8> {
    encode(&closure()).unwrap()
}
