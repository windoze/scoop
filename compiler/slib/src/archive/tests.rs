use scoop_identity::{CapabilityId, ConeIdentity};

use super::*;
use crate::{LogicalMemberKey, MemberStableKey, SlibMember, SlibMemberRecordError, SlibMemberRole};

fn diagnostic_member(key: &[u8], payload: &[u8]) -> Result<SlibMember, SlibMemberRecordError> {
    let capability = CapabilityId::new("org.scoop-lang.test", "diagnostic", 1).unwrap();
    SlibMember::new(
        ConeIdentity::CORE,
        MemberStableKey::DiagnosticAttachment {
            capability: capability.clone(),
            logical_key: LogicalMemberKey::new(key.to_vec()).unwrap(),
        },
        SlibMemberRole::DiagnosticAttachment { capability },
        payload.to_vec(),
    )
}

#[test]
fn archive_has_one_fixed_empty_directory_vector() {
    let archive = CanonicalSlibArchive::write(b"x", Vec::new()).unwrap();
    assert_eq!(
        archive.as_bytes(),
        [
            b"!<arch>\n".as_slice(),
            b"manifest.cbor/  0           0     0     100644  1         `\n".as_slice(),
            b"x\n".as_slice(),
        ]
        .concat()
    );
}

#[test]
fn archive_sorts_members_by_typed_id_and_derives_physical_names() {
    let first_input = diagnostic_member(b"z", b"second").unwrap();
    let second_input = diagnostic_member(b"a", b"first").unwrap();
    let expected = if first_input.record().id() < second_input.record().id() {
        vec![first_input.clone(), second_input.clone()]
    } else {
        vec![second_input.clone(), first_input.clone()]
    };

    let reverse_archive =
        CanonicalSlibArchive::write(b"m", vec![second_input.clone(), first_input.clone()]).unwrap();
    let archive = CanonicalSlibArchive::write(b"m", vec![first_input, second_input]).unwrap();
    assert_eq!(archive, reverse_archive);
    let bytes = archive.as_bytes();
    let first_header = 8 + 60 + 1 + 1;
    assert_eq!(&bytes[first_header..first_header + 16], b"m00000000/      ");
    let first_payload = first_header + 60;
    assert_eq!(
        &bytes[first_payload..first_payload + expected[0].payload().len()],
        expected[0].payload()
    );
    let second_header =
        first_payload + expected[0].payload().len() + (expected[0].payload().len() & 1);
    assert_eq!(
        &bytes[second_header..second_header + 16],
        b"m00000001/      "
    );
    let second_payload = second_header + 60;
    assert_eq!(
        &bytes[second_payload..second_payload + expected[1].payload().len()],
        expected[1].payload()
    );
}

#[test]
fn archive_rejects_duplicate_semantic_member_ids() {
    let member = diagnostic_member(b"same", b"payload").unwrap();
    assert_eq!(
        CanonicalSlibArchive::write(b"manifest", vec![member.clone(), member]),
        Err(ArchiveWriteError::DuplicateMemberId {
            id: diagnostic_member(b"same", b"payload")
                .unwrap()
                .record()
                .id(),
        })
    );
}

#[test]
fn physical_member_names_cover_the_complete_directory_range() {
    assert_eq!(member_name(0).unwrap(), "m00000000");
    assert_eq!(member_name(9).unwrap(), "m00000009");
    assert_eq!(member_name(10).unwrap(), "m00000010");
    assert_eq!(member_name(65_535).unwrap(), "m00065535");
    assert_eq!(
        member_name(65_536),
        Err(ArchiveWriteError::TooManyMembers { actual: 65_537 })
    );
}
