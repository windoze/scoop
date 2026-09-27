use scoop_identity::{CapabilityId, ConeCoordinate};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::encode;

use super::*;
use crate::{
    ArtifactCapabilityProfile, BootstrapManifestError, CompatibilityRecord, ExtensionRequirement,
    LogicalMemberKey, ManifestSectionError, MemberPurposeSet, MemberStableKey, SlibMember,
    SlibMemberRole,
};

fn fingerprints(seed: u8) -> SemanticFingerprintRecord {
    SemanticFingerprintRecord::from_digests(
        HirFingerprint::from_array([seed; 32]),
        MirFingerprint::from_array([seed.wrapping_add(1); 32]),
        LirFingerprint::from_array([seed.wrapping_add(2); 32]),
        crate::FingerprintAvailability::Available(crate::CodeFingerprint::from_array([4; 32])),
        crate::FingerprintAvailability::Available(crate::RuntimeImageFingerprint::from_array(
            [5; 32],
        )),
    )
}

fn compatibility() -> CompatibilityRecord {
    CompatibilityRecord::new(
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
    )
    .unwrap()
}

fn cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn metadata_members() -> Vec<SlibMember> {
    let cone = cone().identity();
    [
        (
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            b"hir".as_slice(),
        ),
        (
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            b"mir".as_slice(),
        ),
        (
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            b"lir".as_slice(),
        ),
    ]
    .into_iter()
    .map(|(key, role, payload)| SlibMember::new(cone, key, role, payload.to_vec()).unwrap())
    .collect()
}

fn manifest(members: &[SlibMember]) -> BootstrapManifest {
    BootstrapManifest::new(
        ProducerRecord::new("dev").unwrap(),
        compatibility(),
        cone(),
        Vec::new(),
        members,
        fingerprints(0x11),
        Vec::new(),
    )
    .unwrap()
}

#[test]
fn manifest_sorts_the_directory_and_is_byte_reproducible() {
    let mut members = metadata_members();
    let first = manifest(&members);
    members.reverse();
    let second = manifest(&members);

    assert_eq!(encode(&first).unwrap(), encode(&second).unwrap());
    assert!(
        first
            .members()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
    );
    assert_eq!(first.artifact_fingerprint(), second.artifact_fingerprint());
    assert_eq!(encode(&first).unwrap()[0], 0xab);
}

#[test]
fn directory_accepts_zero_one_and_multiple_link_objects() {
    let verifier = CapabilityId::new("org.scoop-lang.link-object", "test", 1).unwrap();
    let mut members = metadata_members();
    assert_eq!(manifest(&members).members().len(), 3);

    for logical_key in [b"first".as_slice(), b"second".as_slice()] {
        members.push(
            SlibMember::new(
                cone().identity(),
                MemberStableKey::LinkObject {
                    verifier_capability: verifier.clone(),
                    logical_key: LogicalMemberKey::new(logical_key.to_vec()).unwrap(),
                },
                SlibMemberRole::LinkObject {
                    target_profile: scoop_identity::TargetProfileWireId::darwin_aarch64(),
                    object_format: scoop_identity::ObjectFormatId::macho_relocatable(),
                    verifier_capability: verifier.clone(),
                },
                logical_key.to_vec(),
            )
            .unwrap(),
        );
        assert_eq!(manifest(&members).members().len(), members.len());
    }
}

#[test]
fn optional_and_link_required_blobs_change_only_the_whole_artifact_identity() {
    let mut members = metadata_members();
    let baseline = manifest(&members);
    let capability = CapabilityId::new("org.scoop-lang.test", "blob", 1).unwrap();
    members.push(
        SlibMember::new(
            cone().identity(),
            MemberStableKey::ExtensionBlob {
                capability: capability.clone(),
                logical_key: LogicalMemberKey::new(b"optional".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: capability.clone(),
                requirement: ExtensionRequirement::Optional,
            },
            b"payload".to_vec(),
        )
        .unwrap(),
    );
    members.push(
        SlibMember::new(
            cone().identity(),
            MemberStableKey::ExtensionBlob {
                capability: capability.clone(),
                logical_key: LogicalMemberKey::new(b"link".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability,
                requirement: ExtensionRequirement::Link,
            },
            b"link payload".to_vec(),
        )
        .unwrap(),
    );
    let extended = manifest(&members);

    assert_eq!(
        baseline.semantic_fingerprints(),
        extended.semantic_fingerprints()
    );
    assert_ne!(
        baseline.artifact_fingerprint(),
        extended.artifact_fingerprint()
    );
}

#[test]
fn manifest_rejects_missing_metadata_and_duplicate_semantic_members() {
    let mut members = metadata_members();
    let duplicate = members[0].clone();
    members.push(duplicate);
    assert!(matches!(
        BootstrapManifest::new(
            ProducerRecord::new("dev").unwrap(),
            compatibility(),
            cone(),
            Vec::new(),
            &members,
            fingerprints(1),
            Vec::new(),
        ),
        Err(BootstrapManifestError::DuplicateMember { .. })
    ));

    members = metadata_members();
    members.retain(|member| !matches!(member.record().role(), SlibMemberRole::MirMetadata));
    assert_eq!(
        BootstrapManifest::new(
            ProducerRecord::new("dev").unwrap(),
            compatibility(),
            cone(),
            Vec::new(),
            &members,
            fingerprints(1),
            Vec::new(),
        ),
        Err(BootstrapManifestError::MissingMetadata {
            kind: MetadataKind::Mir,
        })
    );
}

#[test]
fn producer_cone_and_section_boundaries_are_closed() {
    assert!(ProducerRecord::new(&"x".repeat(255)).is_ok());
    assert_eq!(
        ProducerRecord::new(&"x".repeat(256)),
        Err(ProducerRecordError::TooLong { actual: 256 })
    );
    assert_eq!(
        ConeRecord::new(
            ConeCoordinate::reserved_single_file(),
            ConeKind::Executable,
            ConeSourceForm::Manifest,
        ),
        Err(ConeRecordError::SingleFileCoordinateMismatch)
    );
    assert!(
        ConeRecord::new(
            ConeCoordinate::reserved_single_file(),
            ConeKind::Executable,
            ConeSourceForm::SingleFile,
        )
        .is_ok()
    );

    let capability = CapabilityId::new("org.scoop-lang.test", "manifest", 1).unwrap();
    assert_eq!(
        ManifestSection::new(capability, MemberPurposeSet::DIAGNOSTICS, Vec::new(),),
        Err(ManifestSectionError::InvalidPurpose { bits: 8 })
    );
    assert!(matches!(
        ManifestSection::new(
            crate::hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            Vec::new(),
        ),
        Err(ManifestSectionError::KnownCapabilityWrongLocation { .. })
    ));
    assert!(
        ManifestSection::new(
            crate::manifest_single_cone_production_capability(),
            MemberPurposeSet::LINK,
            Vec::new(),
        )
        .is_ok()
    );
    assert!(matches!(
        ManifestSection::new(
            crate::manifest_single_cone_production_capability(),
            MemberPurposeSet::NONE,
            Vec::new(),
        ),
        Err(ManifestSectionError::KnownCapabilityWrongPurpose { .. })
    ));
}
