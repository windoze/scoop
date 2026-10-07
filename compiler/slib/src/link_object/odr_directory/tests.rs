use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    NonEmptyVec, OdrMemberDiscriminator, OdrMemberKey, PackagePath, PersistentExactTypeId,
    PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    SpecializationKey,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn directory_contains_exactly_the_emitted_members_in_canonical_order() {
    let entries = entries();
    let physical = entries.iter().map(|(_, entry)| entry.member).collect();
    let directory =
        CanonicalOdrMemberDirectoryV1::from_members(physical, entries.iter().rev().copied())
            .unwrap();
    assert!(directory.groups.windows(2).all(|p| p[0].group < p[1].group));
    for group in &directory.groups {
        assert!(!group.members.is_empty());
        assert!(group.members.windows(2).all(|p| p[0].member < p[1].member));
    }
    let bytes = encode(&directory).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&bytes).unwrap();
    decoded.validate_against(&directory).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let empty = CanonicalOdrMemberDirectoryV1::from_members(BTreeSet::new(), []).unwrap();
    assert_eq!(encode(&empty).unwrap(), [0x80]);
    decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&[0x80])
        .unwrap()
        .validate_against(&empty)
        .unwrap();
}

#[test]
fn directory_rejects_missing_unexpected_and_repeated_physical_members() {
    let [first, second, ..] = entries();
    assert_eq!(
        CanonicalOdrMemberDirectoryV1::from_members(
            BTreeSet::from([first.1.member, second.1.member]),
            [first],
        ),
        Err(OdrMemberDirectoryProjectionError::MissingMemberFingerprint(
            second.1.member
        )),
    );
    assert_eq!(
        CanonicalOdrMemberDirectoryV1::from_members(BTreeSet::new(), [first]),
        Err(OdrMemberDirectoryProjectionError::UnexpectedMember(
            first.1.member
        )),
    );
    assert_eq!(
        CanonicalOdrMemberDirectoryV1::from_members(
            BTreeSet::from([first.1.member]),
            [first, first],
        ),
        Err(OdrMemberDirectoryProjectionError::DuplicateMember(
            first.1.member
        )),
    );
    let mut entry = first.1;
    entry.role = OdrMemberRole::GeneratedNominal;
    assert_eq!(
        CanonicalOdrMemberDirectoryV1::from_members(
            BTreeSet::from([entry.member]),
            [(first.0, entry)],
        ),
        Err(OdrMemberDirectoryProjectionError::InvalidPhysicalRole {
            member: entry.member,
            role: entry.role,
        }),
    );
}

#[test]
fn directory_reader_rejects_empty_groups_and_noncanonical_member_sets() {
    let directory = directory();
    let mut changed = directory.clone();
    changed.groups[0].members.clear();
    assert_decode_error(
        &changed,
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        },
    );
    let mut changed = directory.clone();
    changed.groups.reverse();
    assert_decode_error(&changed, WireErrorKind::NonCanonicalCbor);
    let mut changed = directory.clone();
    changed.groups.insert(0, changed.groups[0].clone());
    assert_decode_error(&changed, WireErrorKind::NonCanonicalCbor);
    let mut changed = directory.clone();
    changed
        .groups
        .iter_mut()
        .find(|g| g.members.len() == 2)
        .unwrap()
        .members
        .reverse();
    assert_decode_error(&changed, WireErrorKind::NonCanonicalCbor);
    let mut changed = directory.clone();
    let first = changed.groups[0].members[0];
    changed.groups[0].members.insert(0, first);
    assert_decode_error(&changed, WireErrorKind::NonCanonicalCbor);
    let mut changed = directory;
    changed.groups[1].members = vec![changed.groups[0].members[0]];
    assert_decode_error(&changed, WireErrorKind::NonCanonicalCbor);
}

#[test]
fn directory_wire_accepts_the_release_hook_physical_role() {
    let mut directory = directory();
    directory.groups[0].members[0].role = OdrMemberRole::ReleaseHook;
    let bytes = encode(&directory).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&bytes).unwrap();
    decoded.validate_against(&directory).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn directory_reader_checks_record_shapes_roles_and_digest_widths() {
    let mut directory = directory();
    directory.groups.truncate(1);
    directory.groups[0].members.truncate(1);
    let bytes = encode(&directory).unwrap();
    let entry = encode(&directory.groups[0].members[0]).unwrap();
    let start = bytes.windows(entry.len()).position(|v| v == entry).unwrap();
    for field_count in [0xa2, 0xa4] {
        let mut changed = bytes.clone();
        changed[start] = field_count;
        assert!(decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&changed).is_err());
    }
    // A three-field entry starts with a 32-byte ID, then the role field.
    assert_eq!(&entry[..4], &[0xa3, 1, 0x58, 32]);
    assert_eq!(entry[36], 2);
    for role in [0, 2, 17] {
        let mut changed = bytes.clone();
        changed[start + 37] = role;
        assert!(decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&changed).is_err());
    }
    {
        let digest_start = 39;
        assert_eq!(&entry[digest_start..digest_start + 2], &[0x58, 32]);
        let mut changed = bytes.clone();
        changed[start + digest_start + 1] = 31;
        changed.remove(start + digest_start + 2);
        let error = decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&changed).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 32,
                actual: 31
            }
        );
    }
}

#[test]
fn directory_reader_compares_group_member_role_and_abi() {
    let directory = directory();
    let member = directory.groups[0].members[0].member;
    for (changed, error) in [
        (
            {
                let mut changed = directory.clone();
                changed.groups.pop();
                changed
            },
            OdrMemberDirectoryValidationError::GroupSetMismatch,
        ),
        (
            {
                let mut changed = directory.clone();
                changed.groups[0].members[0].role = OdrMemberRole::Layout;
                changed
            },
            OdrMemberDirectoryValidationError::RoleMismatch(member),
        ),
        (
            {
                let mut changed = directory.clone();
                changed.groups[0].members[0].abi = OdrAbiFingerprintV1::from_array([9; 32]);
                changed
            },
            OdrMemberDirectoryValidationError::AbiMismatch(member),
        ),
    ] {
        let decoded =
            decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&encode(&changed).unwrap())
                .unwrap();
        assert_eq!(decoded.validate_against(&directory), Err(error));
    }
    let mut changed = directory.clone();
    let group = changed
        .groups
        .iter_mut()
        .find(|g| g.members.len() == 2)
        .unwrap();
    let id = group.group;
    group.members.pop();
    let decoded =
        decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&encode(&changed).unwrap())
            .unwrap();
    assert_eq!(
        decoded.validate_against(&directory),
        Err(OdrMemberDirectoryValidationError::MemberSetMismatch(id))
    );
}

fn assert_decode_error(directory: &CanonicalOdrMemberDirectoryV1, expected: WireErrorKind) {
    let error =
        decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&encode(directory).unwrap())
            .unwrap_err();
    assert_eq!(error.kind(), &expected);
}

fn directory() -> CanonicalOdrMemberDirectoryV1 {
    let entries = entries();
    CanonicalOdrMemberDirectoryV1::from_members(
        entries.iter().map(|(_, entry)| entry.member).collect(),
        entries,
    )
    .unwrap()
}

fn entries() -> [(OdrGroupId, OdrMemberDirectoryEntryV1); 3] {
    let exact = |name: &str| {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
        PersistentExactTypeId::from_key(&ExactTypeKey::Tuple(NonEmptyVec::from_first(
            exact,
            [exact],
        )))
        .unwrap()
    };
    let first = exact("First");
    let second = exact("Second");
    let group = |exact_type| {
        OdrGroupId::from_key(&SpecializationKey::StructuralType { exact_type }).unwrap()
    };
    [
        (group(first), first),
        (group(first), second),
        (group(second), second),
    ]
    .map(|(group, exact)| {
        let role = OdrMemberRole::TypeDescriptor;
        let member = OdrMemberId::from_key(
            &OdrMemberKey::new(group, role, OdrMemberDiscriminator::ExactType(exact)).unwrap(),
        )
        .unwrap();
        (
            group,
            OdrMemberDirectoryEntryV1 {
                member,
                role,
                abi: OdrAbiFingerprintV1::from_array([3; 32]),
            },
        )
    })
}
