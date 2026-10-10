use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn decoded_protocol_roles_reject_incompatible_wire_subjects() {
    let (surface, foundation) = test_support::standalone();
    let bytes = encode(&surface).unwrap();
    let decoded: DecodedCoreCompilerProtocolSurfaceV1 = decode_canonical(&bytes).unwrap();

    let mut wrong_nominal = decoded.clone();
    wrong_nominal.fundamental_types.0.entries.swap(9, 11);
    assert_eq!(
        wrong_nominal.validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index: 9 })
    );

    let mut wrong_member = decoded.clone();
    wrong_member.option_protocol.0.entries.swap(1, 2);
    assert_eq!(
        wrong_member.validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index: 1 })
    );

    let mut wrong_signature_type = decoded;
    wrong_signature_type
        .foreign_callback_protocol
        .0
        .entries
        .swap(9, 10);
    assert_eq!(
        wrong_signature_type.validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index: 9 })
    );
}
