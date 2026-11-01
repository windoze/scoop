use crate::{LogicalMemberKey, MemberStableKey};
use scoop_identity::{CapabilityId, ConeIdentity};
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
) -> Result<SlibMemberRecord, SlibMemberRoleValidationError> {
    decoded.validate()
}

#[test]
fn member_record_round_trips_through_untrusted_validation() {
    let verifier = capability("object");
    let record = crate::SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::LinkObject {
            verifier_capability: verifier.clone(),
            logical_key: logical_key(b"unit"),
        },
        SlibMemberRole::LinkObject,
        b"object".to_vec(),
    )
    .unwrap()
    .record()
    .clone();
    let decoded = decode_canonical::<DecodedSlibMemberRecord>(&encode(&record).unwrap()).unwrap();

    assert_eq!(validate_member(decoded), Ok(record));
}

#[test]
fn native_record_preserves_its_normalized_source_without_a_duplicate_key() {
    let source = NormalizedNativeSourcePath::new("../shared/原生.c").unwrap();
    let member =
        crate::native_link_object_member(ConeIdentity::CORE, &source, b"object".to_vec()).unwrap();
    let encoded = crate::member::encode_object_members(ConeIdentity::CORE, vec![member]).unwrap();
    let record = encoded[0].record();
    let decoded = decode_canonical::<DecodedSlibMemberRecord>(&encode(record).unwrap()).unwrap();
    assert_eq!(validate_member(decoded).unwrap(), *record);
    assert_eq!(record.role(), &SlibMemberRole::NativeObject { source });
    assert!(record.as_link_member().is_some());
    let invalid = DecodedSlibMemberRole::NativeObject {
        source: "native//wrong.c".to_owned(),
    };
    assert!(matches!(
        invalid.validate(),
        Err(SlibMemberRoleValidationError::NativeSource(_))
    ));
    let missing = decode_canonical::<DecodedSlibMemberRole>(b"\xa1\x00\x07").unwrap_err();
    assert_eq!(
        missing.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1
        }
    );
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
    let old_record = decode_canonical::<DecodedSlibMemberRecord>(
        b"\xa5\x01\x00\x02\x00\x03\x00\x04\x00\x05\x00",
    )
    .unwrap_err();
    assert_eq!(
        old_record.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 5
        }
    );
    let short_id = decode_canonical::<SlibMemberId>(b"\x40").unwrap_err();
    assert_eq!(
        short_id.kind(),
        &WireErrorKind::InvalidLength {
            expected: 32,
            actual: 0
        }
    );
    let unknown = decode_canonical::<DecodedSlibMemberRole>(b"\xa1\x00\x09").unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let extra_field =
        decode_canonical::<DecodedSlibMemberRole>(b"\xa3\x00\x01\x01\x01\x02\x00").unwrap_err();
    assert_eq!(
        extra_field.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 3,
        }
    );
}
