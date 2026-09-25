use scoop_identity::{CapabilityId, ConeIdentity, ObjectFormatId, TargetProfileWireId};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

fn capability(name: &str) -> CapabilityId {
    CapabilityId::new("org.scoop-lang.test", name, 1).unwrap()
}

fn logical_key(bytes: &[u8]) -> LogicalMemberKey {
    LogicalMemberKey::new(bytes.to_vec()).unwrap()
}

fn validate_member(
    decoded: DecodedSlibMemberRecord,
) -> Result<SlibMemberRecord, SlibMemberRecordValidationError> {
    decoded.validate(ConeIdentity::CORE)
}

#[test]
fn member_record_round_trips_through_untrusted_validation() {
    let verifier = capability("object");
    let record = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: verifier.clone(),
            logical_key: logical_key(b"unit"),
        },
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            object_format: ObjectFormatId::macho_relocatable(),
            verifier_capability: verifier,
        },
        b"object",
    )
    .unwrap();
    let decoded = decode_canonical::<DecodedSlibMemberRecord>(&encode(&record).unwrap()).unwrap();

    assert_eq!(validate_member(decoded), Ok(record));
}

#[test]
fn decoded_record_recomputes_member_id() {
    let record = SlibMemberRecord::new(
        ConeIdentity::CORE,
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata,
        b"hir",
    )
    .unwrap();
    let mut decoded =
        decode_canonical::<DecodedSlibMemberRecord>(&encode(&record).unwrap()).unwrap();
    decoded.id.0[0] ^= 1;

    assert!(matches!(
        validate_member(decoded),
        Err(SlibMemberRecordValidationError::IdMismatch { .. })
    ));
}

#[test]
fn decoded_metadata_role_requires_initial_wire_schema() {
    let role = decode_canonical::<DecodedSlibMemberRole>(b"\xa2\x00\x01\x01\x02").unwrap();
    assert_eq!(
        role.validate(),
        Err(SlibMemberRoleValidationError::UnsupportedWireSchema { actual: 2 })
    );
}

#[test]
fn decoded_extension_role_rejects_other_purpose_bits() {
    let capability = capability("blob");
    let encoded_capability = encode(&capability).unwrap();
    let bytes = [
        b"\xa3\x00\x06\x01".as_slice(),
        encoded_capability.as_slice(),
        b"\x02\x02".as_slice(),
    ]
    .concat();
    let role = decode_canonical::<DecodedSlibMemberRole>(&bytes).unwrap();

    assert_eq!(
        role.validate(),
        Err(SlibMemberRoleValidationError::InvalidExtensionRequirement { bits: 2 })
    );
}

#[test]
fn closed_member_sums_reject_unknown_tags_and_shapes() {
    let unknown = decode_canonical::<DecodedMemberStableKey>(b"\xa1\x00\x07").unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 7 });

    let extra_field =
        decode_canonical::<DecodedMemberStableKey>(b"\xa2\x00\x01\x01\x00").unwrap_err();
    assert_eq!(
        extra_field.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );
}
