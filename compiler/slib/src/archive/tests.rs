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
    assert_eq!(member_name(0).as_deref(), Some("m00000000"));
    assert_eq!(member_name(9).as_deref(), Some("m00000009"));
    assert_eq!(member_name(10).as_deref(), Some("m00000010"));
    assert_eq!(member_name(65_535).as_deref(), Some("m00065535"));
    assert_eq!(member_name(65_536), None);
}

#[test]
fn reader_validates_manifest_directory_and_member_ranges() {
    let first = diagnostic_member(b"first", b"one").unwrap();
    let second = diagnostic_member(b"second", b"two!").unwrap();
    let mut records = vec![first.record().clone(), second.record().clone()];
    records.sort_unstable_by_key(SlibMemberRecord::id);
    let archive =
        CanonicalSlibArchive::write(b"canonical manifest", vec![second.clone(), first.clone()])
            .unwrap();

    let manifest = ManifestArchive::open(archive.as_bytes()).unwrap();
    assert_eq!(manifest.manifest(), b"canonical manifest");
    let decoded = manifest.validate_directory(&records).unwrap();
    assert_eq!(decoded.manifest(), b"canonical manifest");
    assert_eq!(
        decoded.member_ids().collect::<Vec<_>>(),
        records.iter().map(SlibMemberRecord::id).collect::<Vec<_>>()
    );
    let payload = |id| {
        let index = decoded
            .members
            .binary_search_by_key(&id, |(member_id, _)| *member_id)
            .unwrap();
        &decoded.input[decoded.members[index].1.clone()]
    };
    assert_eq!(payload(first.record().id()), first.payload());
    assert_eq!(payload(second.record().id()), second.payload());
}

#[test]
fn reader_rejects_noncanonical_container_bytes() {
    let archive = CanonicalSlibArchive::write(b"x", Vec::new()).unwrap();

    let mut bad_magic = archive.as_bytes().to_vec();
    bad_magic[0] = b'?';
    assert_eq!(
        ManifestArchive::open(&bad_magic),
        Err(ArchiveReadError::BadMagic)
    );

    let mut bad_header = archive.as_bytes().to_vec();
    bad_header[8 + 16] = b'1';
    assert_eq!(
        ManifestArchive::open(&bad_header),
        Err(ArchiveReadError::NonCanonicalHeader {
            member: ArchiveMemberOrdinal::Manifest,
        })
    );

    let mut bad_padding = archive.as_bytes().to_vec();
    *bad_padding.last_mut().unwrap() = b' ';
    assert_eq!(
        ManifestArchive::open(&bad_padding),
        Err(ArchiveReadError::InvalidPadding {
            member: ArchiveMemberOrdinal::Manifest,
        })
    );
}

#[test]
fn reader_rejects_directory_order_name_digest_and_total_length_corruption() {
    let first = diagnostic_member(b"first", b"one").unwrap();
    let second = diagnostic_member(b"second", b"two").unwrap();
    let mut records = vec![first.record().clone(), second.record().clone()];
    records.sort_unstable_by_key(SlibMemberRecord::id);
    let archive = CanonicalSlibArchive::write(b"manifest", vec![first, second]).unwrap();

    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        ManifestArchive::open(archive.as_bytes())
            .unwrap()
            .validate_directory(&reversed),
        Err(ArchiveReadError::NonIncreasingDirectory {
            first_index: 0,
            second_index: 1,
        })
    );

    let mut bad_name = archive.as_bytes().to_vec();
    let first_header = ManifestArchive::open(&bad_name).unwrap().next_header;
    bad_name[first_header] = b'x';
    assert_eq!(
        ManifestArchive::open(&bad_name)
            .unwrap()
            .validate_directory(&records),
        Err(ArchiveReadError::NonCanonicalHeader {
            member: ArchiveMemberOrdinal::Directory(0),
        })
    );

    let mut bad_payload = archive.as_bytes().to_vec();
    let first_header = ManifestArchive::open(&bad_payload).unwrap().next_header;
    bad_payload[first_header + 60] ^= 1;
    assert_eq!(
        ManifestArchive::open(&bad_payload)
            .unwrap()
            .validate_directory(&records),
        Err(ArchiveReadError::MemberDigestMismatch {
            id: records[0].id()
        })
    );

    let mut trailing = archive.as_bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        ManifestArchive::open(&trailing)
            .unwrap()
            .validate_directory(&records),
        Err(ArchiveReadError::PredictedLengthMismatch {
            predicted: archive.as_bytes().len() as u64,
            actual: archive.as_bytes().len() as u64 + 1,
        })
    );
}
