use scoop_identity::ConeCoordinate;

use super::*;
use crate::{
    CompatibilityRecord, ConeKind, ConeRecord, ConeSourceForm, HirFingerprint, LirFingerprint,
    MemberStableKey, MirFingerprint, ProducerRecord, SemanticFingerprintRecord, SlibDiagnostic,
    SlibErrorCode, SlibMemberRecord, SlibMemberRole, SlibPrimaryOrigin,
};

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

fn members() -> Vec<SlibMember> {
    let cone = ConeCoordinate::reserved_core().identity().unwrap();
    [
        (
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            b"unique HIR payload".as_slice(),
        ),
        (
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            b"unique MIR payload".as_slice(),
        ),
        (
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            b"unique LIR payload".as_slice(),
        ),
    ]
    .into_iter()
    .map(|(key, role, payload)| SlibMember::new(cone, key, role, payload.to_vec()).unwrap())
    .collect()
}

fn manifest(members: &[SlibMember]) -> BootstrapManifest {
    BootstrapManifest::new(
        ProducerRecord::new("dev").unwrap(),
        CompatibilityRecord::new(
            selection(),
            crate::ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        )
        .unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        Vec::new(),
        members,
        SemanticFingerprintRecord::from_digests(
            HirFingerprint::from_array([1; 32]),
            MirFingerprint::from_array([2; 32]),
            LirFingerprint::from_array([3; 32]),
            crate::FingerprintAvailability::Available(crate::CodeFingerprint::from_array([4; 32])),
            crate::FingerprintAvailability::Available(crate::RuntimeImageFingerprint::from_array(
                [5; 32],
            )),
        ),
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn bootstrap_writer_and_envelope_reader_round_trip() {
    let members = members();
    let expected_manifest = manifest(&members);
    let archive = CanonicalSlibArchive::write_bootstrap(&expected_manifest, members).unwrap();
    let envelope = DecodedSlibEnvelope::open(archive.as_bytes(), selection()).unwrap();

    assert_eq!(envelope.manifest, expected_manifest);
    assert_eq!(
        envelope.archive.member_ids().collect::<Vec<_>>(),
        envelope
            .manifest
            .members()
            .iter()
            .map(SlibMemberRecord::id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn bootstrap_writer_is_reproducible_and_rejects_directory_mismatch() {
    let members = members();
    let manifest = manifest(&members);
    let forward = CanonicalSlibArchive::write_bootstrap(&manifest, members.clone()).unwrap();
    let reverse =
        CanonicalSlibArchive::write_bootstrap(&manifest, members.iter().cloned().rev().collect())
            .unwrap();
    assert_eq!(forward, reverse);

    let mut changed = members;
    let member = changed.pop().unwrap();
    let replacement = SlibMember::new(
        manifest.cone().identity(),
        member.record().stable_key().clone(),
        member.record().role().clone(),
        b"different payload".to_vec(),
    )
    .unwrap();
    changed.push(replacement);
    assert!(matches!(
        CanonicalSlibArchive::write_bootstrap(&manifest, changed),
        Err(SlibWriteError::DirectoryRecordMismatch { .. })
    ));
}

#[test]
fn envelope_rejects_manifest_and_member_corruption() {
    let members = members();
    let manifest = manifest(&members);
    let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();

    let mut bad_manifest = archive.as_bytes().to_vec();
    let magic = bad_manifest
        .windows(b"SCOOPSLIB".len())
        .position(|window| window == b"SCOOPSLIB")
        .unwrap();
    bad_manifest[magic] = b'X';
    let error = DecodedSlibEnvelope::open(&bad_manifest, selection())
        .expect_err("corrupted manifest magic must fail");
    assert!(matches!(
        &error,
        SlibReadError::Manifest(error)
            if **error == BootstrapManifestValidationError::BadMagic
    ));
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), SlibErrorCode::WireNonCanonical);
    assert_eq!(diagnostic.path().to_string(), "$.1");
    assert_eq!(
        diagnostic.primary_origin(),
        Some(&SlibPrimaryOrigin::Manifest)
    );

    let mut bad_member = archive.as_bytes().to_vec();
    let payload = b"unique HIR payload";
    let offset = bad_member
        .windows(payload.len())
        .position(|window| window == payload)
        .unwrap();
    bad_member[offset] ^= 1;
    let error = DecodedSlibEnvelope::open(&bad_member, selection())
        .expect_err("corrupted member payload must fail");
    assert!(matches!(
        &error,
        SlibReadError::Directory(ArchiveReadError::MemberDigestMismatch { .. })
    ));
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code(), SlibErrorCode::FingerprintMismatch);
    assert!(matches!(
        diagnostic.primary_origin(),
        Some(SlibPrimaryOrigin::Member(_))
    ));
}
