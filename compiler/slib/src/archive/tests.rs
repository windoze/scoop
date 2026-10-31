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

fn open_archive(input: &[u8]) -> Result<ManifestArchive<'_>, ArchiveReadError> {
    ManifestArchive::open(input)
}

#[test]
fn object_storage_is_compressed_while_cache_outputs_and_readers_use_native_bytes() {
    let capability = CapabilityId::new("org.scoop-lang.test", "object", 1).unwrap();
    let mut members = vec![diagnostic_member(b"log", b"diagnostic").unwrap()];
    let mut expected = std::collections::BTreeMap::new();
    let mut records = Vec::new();
    for bytes in [b"odd".as_slice(), b"even".as_slice()] {
        let member = SlibMember::new(
            ConeIdentity::CORE,
            MemberStableKey::LinkObject {
                verifier_capability: capability.clone(),
                logical_key: crate::LogicalMemberKey::new(bytes.to_vec()).unwrap(),
            },
            SlibMemberRole::LinkObject,
            bytes.to_vec(),
        )
        .unwrap();
        assert_ne!(member.payload(), bytes);
        assert_eq!(
            member.record().sha256(),
            scoop_wire::sha256(member.payload())
        );
        expected.insert(member.record().sha256(), bytes);
        records.push(member.record().clone());
        members.push(member);
    }
    records.extend(
        members
            .iter()
            .filter(|member| !member.record().role().is_object())
            .map(|member| member.record().clone()),
    );
    records.sort_unstable_by_key(SlibMemberRecord::id);
    let archive = CanonicalSlibArchive::write(b"m", members).unwrap();
    assert_eq!(
        archive
            .object_payloads()
            .collect::<std::collections::BTreeMap<_, _>>(),
        expected
    );
    let decoded = open_archive(archive.as_bytes())
        .unwrap()
        .validate_directory(&records)
        .unwrap();
    for record in &records {
        if let Some(expected) = expected.get(&record.sha256()) {
            let bytes = decoded.member(record.id()).unwrap().unwrap();
            assert_eq!(bytes, *expected);
            assert!(std::ptr::eq(
                bytes,
                decoded.member(record.id()).unwrap().unwrap()
            ));
        }
    }
}

fn validate_archive<'input>(
    input: &'input [u8],
    records: &[SlibMemberRecord],
) -> Result<DecodedArchive<'input>, ArchiveReadError> {
    ManifestArchive::open(input)?.validate_directory(records)
}

#[test]
fn archive_sorts_members_and_stores_payloads_without_per_member_headers() {
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
    assert_eq!(&bytes[..8], b"SCOOPSLB");
    let manifest_length = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let first_payload = 16 + manifest_length;
    assert_eq!(first_payload, open_archive(bytes).unwrap().next_payload);
    assert_eq!(
        &bytes[first_payload..first_payload + expected[0].payload().len()],
        expected[0].payload()
    );
    let second_payload = first_payload + expected[0].payload().len();
    assert_eq!(&bytes[second_payload..], expected[1].payload());
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
fn empty_directory_uses_only_the_fixed_header_and_manifest() {
    for manifest in [b"".as_slice(), b"odd", b"even"] {
        let archive = CanonicalSlibArchive::write(manifest, Vec::new()).unwrap();
        let bytes = archive.as_bytes();
        assert_eq!(
            bytes.len(),
            16 + u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize
        );
        let decoded = open_archive(bytes)
            .unwrap()
            .validate_directory(&[])
            .unwrap();
        assert_eq!(decoded.manifest(), manifest);
        assert_eq!(decoded.member_ids().len(), 0);
    }
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
    let payload = |id| decoded.member(id).unwrap().unwrap();
    assert_eq!(payload(first.record().id()), b"one");
    assert_eq!(payload(second.record().id()), b"two!");
}

#[test]
fn reader_rejects_invalid_magic_truncation_and_overflow() {
    let archive = CanonicalSlibArchive::write(b"xy", Vec::new()).unwrap();
    let mut bytes = archive.as_bytes().to_vec();
    bytes[0] = b'?';
    assert_eq!(open_archive(&bytes), Err(ArchiveReadError::BadMagic));
    assert_eq!(open_archive(b"!<arch>\n"), Err(ArchiveReadError::BadMagic));
    assert_eq!(
        open_archive(&archive.as_bytes()[..15]),
        Err(ArchiveReadError::TruncatedHeader {
            member: ArchiveMemberOrdinal::Manifest
        })
    );
    assert_eq!(
        open_archive(&archive.as_bytes()[..archive.as_bytes().len() - 1]),
        Err(ArchiveReadError::TruncatedPayload {
            member: ArchiveMemberOrdinal::Manifest
        })
    );
    let mut overflowing = archive.as_bytes().to_vec();
    overflowing[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        open_archive(&overflowing),
        Err(ArchiveReadError::LengthOverflow)
    );
}

#[test]
fn reader_indexes_the_directory_and_checks_only_requested_payload_ranges() {
    let first = diagnostic_member(b"first", b"one").unwrap();
    let second = diagnostic_member(b"second", b"two").unwrap();
    let mut records = vec![first.record().clone(), second.record().clone()];
    records.sort_unstable_by_key(SlibMemberRecord::id);
    let archive = CanonicalSlibArchive::write(b"manifest", vec![first, second]).unwrap();
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        validate_archive(archive.as_bytes(), &reversed),
        Err(ArchiveReadError::NonIncreasingDirectory {
            first_index: 0,
            second_index: 1
        })
    );
    assert_eq!(
        member_positions(&records, u64::MAX),
        Err(ArchiveReadError::LengthOverflow)
    );
    let truncated = &archive.as_bytes()[..archive.as_bytes().len() - 1];
    let decoded = validate_archive(truncated, &records).unwrap();
    assert!(decoded.member(records[0].id()).unwrap().is_some());
    assert_eq!(
        decoded.member(records[1].id()),
        Err(ArchiveReadError::TruncatedPayload {
            member: ArchiveMemberOrdinal::Directory(1)
        })
    );
}
