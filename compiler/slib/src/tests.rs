//! `.slib` envelope tests: canonical archive golden bytes, member
//! directory encoding, manifest round-trip, corruption rejection and
//! bitwise reproducibility.

use scoop_identity::capability::{generated_c_bridge_link_object_v1, scoop_lir_link_object_v1};
use scoop_identity::{
    CapabilityId, ConeCoordinate, ConeIdentity, Digest256, ObjectFormatId, TargetProfileWireId,
};
use scoop_manifest::ConeKind;

use crate::archive::{ArchiveError, read_archive, write_archive};
use crate::artifact::{
    ManifestCoreTemplate, SlibBuilder, SlibError, ValidatedGraphArtifact, plain_logical_key,
};
use crate::limits::SlibDecodeLimits;
use crate::manifest::{DependencyRecord, MANIFEST_MEMBER_NAME, ManifestCore};
use crate::member::{LogicalKey, MemberPurposeSet, MemberStableKey, SlibMemberId, SlibMemberRole};

fn template() -> ManifestCoreTemplate {
    ManifestCoreTemplate {
        container_version: 1,
        hir_wire_schema: 1,
        mir_wire_schema: 1,
        lir_wire_schema: 1,
        producer_compiler_version: "0.0.0 (test)".to_owned(),
        language_abi: 1,
        runtime_abi: Digest256::from_bytes([1; 32]),
        identity_schema_version: 1,
        coordinate: ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap(),
        kind: ConeKind::Library,
        dependencies: vec![DependencyRecord {
            coordinate: ConeCoordinate::new("org.foo", "bar", "1.2.3").unwrap(),
            cone_identity: ConeIdentity::of(
                &ConeCoordinate::new("org.foo", "bar", "1.2.3").unwrap(),
            ),
            hir_semantic_fingerprint: Digest256::from_bytes([2; 32]),
            mir_semantic_fingerprint: Digest256::from_bytes([3; 32]),
            lir_semantic_fingerprint: Digest256::from_bytes([4; 32]),
        }],
        target_profile: TargetProfileWireId::darwin_aarch64_v1(),
        target_profile_fingerprint: Digest256::from_bytes([5; 32]),
        backend_profile_fingerprint: Digest256::from_bytes([6; 32]),
    }
}

fn add_standard_members(builder: &mut SlibBuilder) {
    builder
        .add_member(
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata { wire_schema: 1 },
            b"hir-payload".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata { wire_schema: 1 },
            b"mir-payload".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata { wire_schema: 1 },
            b"lir-payload".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::LinkObject {
                verifier_capability: scoop_lir_link_object_v1(),
                logical_key: plain_logical_key("unit-1").unwrap(),
            },
            SlibMemberRole::LinkObject {
                target_profile: TargetProfileWireId::darwin_aarch64_v1(),
                object_format: ObjectFormatId::mach_o_relocatable_v1(),
                verifier_capability: scoop_lir_link_object_v1(),
            },
            vec![0x11, 0x22, 0x33], // odd length to exercise the pad byte
        )
        .unwrap();
}

fn build_sample() -> Vec<u8> {
    let mut builder = SlibBuilder::new(template());
    add_standard_members(&mut builder);
    builder.finish().unwrap()
}

#[test]
fn canonical_archive_golden_bytes() {
    let data = write_archive(&[("a", &[1u8]), ("b", &[1u8, 2u8, 3u8])]);
    assert_eq!(&data[..8], b"!<arch>\n");
    // Header for "a": name "a/" + 14 spaces, zeros, mode, size "1".
    let header_a = &data[8..68];
    assert_eq!(&header_a[0..16], b"a/              ");
    assert_eq!(&header_a[16..28], b"0           ");
    assert_eq!(&header_a[28..34], b"0     ");
    assert_eq!(&header_a[34..40], b"0     ");
    assert_eq!(&header_a[40..48], b"100644  ");
    assert_eq!(&header_a[48..58], b"1         ");
    assert_eq!(&header_a[58..60], b"`\n");
    assert_eq!(data[68], 1);
    // Payload of one byte is odd → pad byte follows.
    assert_eq!(data[69], b'\n');
    // Header for "b" starts at 70 and occupies 60 bytes.
    let header_b = &data[70..130];
    assert_eq!(&header_b[0..16], b"b/              ");
    assert_eq!(&header_b[48..58], b"3         ");
    assert_eq!(&data[130..133], [1, 2, 3]);
    assert_eq!(data.len(), 134);

    let members = read_archive(&data).unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].name, "a");
    assert_eq!(members[0].payload, &[1]);
    assert_eq!(members[1].name, "b");
    assert_eq!(members[1].payload, &[1, 2, 3]);
}

#[test]
fn archive_rejects_non_canonical_and_reserved() {
    // Missing magic.
    assert_eq!(
        read_archive(b"garbage"),
        Err(ArchiveError::MissingGlobalMagic)
    );
    // Reserved symbol-table member.
    let data = write_archive(&[("__.SYMDEF", &[0u8])]);
    assert!(matches!(
        read_archive(&data).unwrap_err(),
        ArchiveError::ReservedMemberName(_)
    ));
    // Long-name table and thin-archive markers are rejected under either
    // error class: both the reserved-name scan and the canonical name
    // field parse refuse them.
    for marker in ["//", "/"] {
        let data = write_archive(&[(marker, &[0u8; 4])]);
        assert!(matches!(
            read_archive(&data).unwrap_err(),
            ArchiveError::ReservedMemberName(_) | ArchiveError::NonCanonicalField { .. }
        ));
    }

    // Re-spelled mtime field ("00" instead of "0").
    let canonical = write_archive(&[("a", &[1u8])]);
    let mut tampered = canonical.clone();
    tampered[16] = b'0';
    tampered[17] = b'0';
    assert!(matches!(
        read_archive(&tampered).unwrap_err(),
        ArchiveError::NonCanonicalField { .. }
    ));

    // Size with leading zero.
    let mut tampered = canonical.clone();
    tampered[48] = b'0';
    tampered[49] = b'1';
    assert!(matches!(
        read_archive(&tampered).unwrap_err(),
        ArchiveError::NonCanonicalField { .. }
    ));

    // Truncated payload.
    assert!(matches!(
        read_archive(&canonical[..canonical.len() - 1]).unwrap_err(),
        ArchiveError::BadPadByte { .. } | ArchiveError::TruncatedPayload { .. }
    ));
}

#[test]
fn member_id_is_derived_from_cone_and_key() {
    let cone = ConeIdentity::of(&ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap());
    let id = SlibMemberId::of(cone, &MemberStableKey::HirMetadata);
    // Same key under another cone identity is another member.
    let other_cone = ConeIdentity::of(&ConeCoordinate::new("dev.example", "app", "0.1.1").unwrap());
    assert_ne!(
        id,
        SlibMemberId::of(other_cone, &MemberStableKey::HirMetadata)
    );
    // Logical keys bound the id.
    let key_a = plain_logical_key("a").unwrap();
    let key_b = plain_logical_key("b").unwrap();
    let cap = scoop_lir_link_object_v1();
    assert_ne!(
        SlibMemberId::of(
            cone,
            &MemberStableKey::LinkObject {
                verifier_capability: cap.clone(),
                logical_key: key_a.clone(),
            }
        ),
        SlibMemberId::of(
            cone,
            &MemberStableKey::LinkObject {
                verifier_capability: cap,
                logical_key: key_b,
            }
        )
    );
}

#[test]
fn logical_key_bounds() {
    assert!(LogicalKey::new(Vec::new()).is_err());
    assert!(LogicalKey::new(vec![0u8; 4096]).is_ok());
    assert!(LogicalKey::new(vec![0u8; 4097]).is_err());
}

#[test]
fn role_key_pairing_rules() {
    let id = SlibMemberId::from_validated([0; 32]);
    // Metadata keys only pair with their own role.
    assert!(
        SlibMemberRole::HirMetadata { wire_schema: 1 }
            .pairing(&MemberStableKey::HirMetadata, id)
            .is_ok()
    );
    assert!(
        SlibMemberRole::MirMetadata { wire_schema: 1 }
            .pairing(&MemberStableKey::HirMetadata, id)
            .is_err()
    );
    // Link object capability must match the key.
    let key = MemberStableKey::LinkObject {
        verifier_capability: scoop_lir_link_object_v1(),
        logical_key: plain_logical_key("x").unwrap(),
    };
    assert!(
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::darwin_aarch64_v1(),
            object_format: ObjectFormatId::mach_o_relocatable_v1(),
            verifier_capability: scoop_lir_link_object_v1(),
        }
        .pairing(&key, id)
        .is_ok()
    );
    assert!(
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::darwin_aarch64_v1(),
            object_format: ObjectFormatId::mach_o_relocatable_v1(),
            verifier_capability: generated_c_bridge_link_object_v1(),
        }
        .pairing(&key, id)
        .is_err()
    );
    // Extension blob purpose sets: only none or exactly Link.
    let blob_key = MemberStableKey::ExtensionBlob {
        capability: CapabilityId::new("example.test", "blob", 1).unwrap(),
        logical_key: plain_logical_key("x").unwrap(),
    };
    assert!(
        SlibMemberRole::ExtensionBlob {
            capability: CapabilityId::new("example.test", "blob", 1).unwrap(),
            required_for: 0,
        }
        .pairing(&blob_key, id)
        .is_ok()
    );
    assert!(
        SlibMemberRole::ExtensionBlob {
            capability: CapabilityId::new("example.test", "blob", 1).unwrap(),
            required_for: MemberPurposeSet::LINK,
        }
        .pairing(&blob_key, id)
        .is_ok()
    );
    assert!(
        SlibMemberRole::ExtensionBlob {
            capability: CapabilityId::new("example.test", "blob", 1).unwrap(),
            required_for: MemberPurposeSet::GRAPH,
        }
        .pairing(&blob_key, id)
        .is_err()
    );
}

/// Small shim so tests can exercise pairing validation directly.
trait Pairing {
    fn pairing(
        &self,
        key: &MemberStableKey,
        id: SlibMemberId,
    ) -> Result<(), crate::member::MemberError>;
}

impl Pairing for SlibMemberRole {
    fn pairing(
        &self,
        key: &MemberStableKey,
        id: SlibMemberId,
    ) -> Result<(), crate::member::MemberError> {
        crate::member::SlibMemberRecord::new(id, key.clone(), self.clone(), 0, Digest256::ZERO)
            .map(|_| ())
    }
}

#[test]
fn manifest_round_trip_and_identity_validation() {
    let data = build_sample();
    let limits = SlibDecodeLimits::strict();
    let graph = ValidatedGraphArtifact::read(&data, &limits).expect("decodes");
    let manifest: &ManifestCore = graph.manifest();
    assert_eq!(manifest.coordinate.display(), "dev.example:app:0.1.0");
    assert_eq!(manifest.kind, ConeKind::Library);
    assert_eq!(manifest.dependencies.len(), 1);
    assert_eq!(manifest.members.len(), 4);
    // The artifact fingerprint matches the recomputed value.
    assert_eq!(
        manifest.artifact_fingerprint,
        manifest.compute_artifact_fingerprint()
    );
}

#[test]
fn builder_is_bitwise_deterministic() {
    let first = build_sample();
    let second = build_sample();
    assert_eq!(first, second);
    // Reordering member insertion does not change the bytes: directory
    // order is by SlibMemberId, not insertion.
    let mut builder = SlibBuilder::new(template());
    builder
        .add_member(
            MemberStableKey::LinkObject {
                verifier_capability: scoop_lir_link_object_v1(),
                logical_key: plain_logical_key("unit-1").unwrap(),
            },
            SlibMemberRole::LinkObject {
                target_profile: TargetProfileWireId::darwin_aarch64_v1(),
                object_format: ObjectFormatId::mach_o_relocatable_v1(),
                verifier_capability: scoop_lir_link_object_v1(),
            },
            vec![0x11, 0x22, 0x33],
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata { wire_schema: 1 },
            b"lir-payload".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata { wire_schema: 1 },
            b"mir-payload".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata { wire_schema: 1 },
            b"hir-payload".to_vec(),
        )
        .unwrap();
    assert_eq!(builder.finish().unwrap(), first);
}

#[test]
fn physical_names_follow_directory_order() {
    let data = build_sample();
    let members = read_archive(&data).unwrap();
    assert_eq!(members[0].name, MANIFEST_MEMBER_NAME);
    for (ordinal, member) in members.iter().skip(1).enumerate() {
        assert_eq!(member.name, format!("m{ordinal:08}"));
    }
}

#[test]
fn corruption_is_rejected_without_panic() {
    let data = build_sample();
    let limits = SlibDecodeLimits::strict();

    // Flip a byte inside each region: magic, manifest, payload.
    for index in [0usize, 9, 60, data.len() - 1] {
        let mut tampered = data.clone();
        tampered[index] ^= 0x40;
        // Every tamper either fails to decode or produces a mismatched
        // fingerprint/hash; it must never panic.
        let _ = ValidatedGraphArtifact::read(&tampered, &limits);
    }

    // Truncations at several lengths.
    for length in [0usize, 7, 8, 68, data.len() / 2, data.len() - 1] {
        let _ = ValidatedGraphArtifact::read(&data[..length.min(data.len())], &limits);
    }

    // Payload hash mismatch: find a payload byte and flip it.
    let members = read_archive(&data).unwrap();
    let link_payload_start = data
        .windows(4)
        .position(|window| window == [0x11, 0x22, 0x33, b'\n'])
        .expect("link payload with pad");
    let mut tampered = data.clone();
    tampered[link_payload_start + 1] ^= 0xFF;
    assert!(matches!(
        ValidatedGraphArtifact::read(&tampered, &limits).unwrap_err(),
        SlibError::PayloadHashMismatch { .. }
    ));
    let _ = members;
}

#[test]
fn duplicate_member_id_is_rejected_by_builder() {
    let mut builder = SlibBuilder::new(template());
    add_standard_members(&mut builder);
    // Re-adding the same stable key under the same role duplicates the id.
    let result = builder.add_member(
        MemberStableKey::HirMetadata,
        SlibMemberRole::HirMetadata { wire_schema: 1 },
        b"another".to_vec(),
    );
    assert!(matches!(result, Err(SlibError::Limits(_))));
}

#[test]
fn budget_limits_are_enforced() {
    let data = build_sample();
    // A budget below the actual size must reject, not truncate.
    let mut tiny = SlibDecodeLimits::strict();
    tiny.archive_max_bytes = 64;
    assert!(matches!(
        ValidatedGraphArtifact::read(&data, &tiny).unwrap_err(),
        SlibError::Limits(_)
    ));
}

#[test]
fn optional_blob_changes_only_the_artifact_fingerprint() {
    let base = build_sample();
    let mut with_blob = SlibBuilder::new(template());
    add_standard_members(&mut with_blob);
    with_blob
        .add_member(
            MemberStableKey::DiagnosticAttachment {
                capability: CapabilityId::new("example.test", "notes", 3).unwrap(),
                logical_key: plain_logical_key("notes").unwrap(),
            },
            SlibMemberRole::DiagnosticAttachment {
                capability: CapabilityId::new("example.test", "notes", 3).unwrap(),
            },
            b"opaque diagnostic attachment".to_vec(),
        )
        .unwrap();
    let extended = with_blob.finish().unwrap();
    assert_ne!(base, extended);

    let limits = SlibDecodeLimits::strict();
    let base_view = ValidatedGraphArtifact::read(&base, &limits).unwrap();
    let extended_view = ValidatedGraphArtifact::read(&extended, &limits).unwrap();
    assert_eq!(extended_view.members().len(), base_view.members().len() + 1);
    assert_ne!(
        extended_view.artifact_fingerprint(),
        base_view.artifact_fingerprint()
    );
    // Identity-bearing manifest fields are unchanged.
    assert_eq!(extended_view.coordinate(), base_view.coordinate());
}
