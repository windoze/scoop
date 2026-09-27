use scoop_identity::{CapabilityId, ConeCoordinate};

use super::*;
use crate::{
    BootstrapManifest, CanonicalSlibArchive, CompatibilityRecord, ConeRecord, ExtensionRequirement,
    HirFingerprint, LirFingerprint, LogicalMemberKey, ManifestSection, MemberPurposeSet,
    MemberStableKey, MirFingerprint, ProducerRecord, SemanticFingerprintRecord, SlibMember,
    SlibMemberRole,
};

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

fn metadata_members() -> Vec<SlibMember> {
    let cone = ConeCoordinate::reserved_core().identity().unwrap();
    [
        (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata),
        (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata),
        (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata),
    ]
    .into_iter()
    .map(|(key, role)| SlibMember::new(cone, key, role, Vec::new()).unwrap())
    .collect()
}

fn manifest(
    dependencies: Vec<DependencyRecord>,
    members: &[SlibMember],
    sections: Vec<ManifestSection>,
) -> BootstrapManifest {
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
        dependencies,
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
        sections,
    )
    .unwrap()
}

fn archive(manifest: &BootstrapManifest, members: Vec<SlibMember>) -> CanonicalSlibArchive {
    CanonicalSlibArchive::write_bootstrap(manifest, members).unwrap()
}

#[test]
fn graph_proof_exposes_only_node_identity_and_dependency_records() {
    let members = metadata_members();
    let dependency = DependencyRecord::new(
        ConeCoordinate::new("example", "dependency", "0.1.0").unwrap(),
        HirFingerprint::from_array([4; 32]),
        MirFingerprint::from_array([5; 32]),
        LirFingerprint::from_array([6; 32]),
    )
    .unwrap();
    let expected_dependency = dependency.clone();
    let archive = archive(&manifest(vec![dependency], &members, Vec::new()), members);
    let graph = DecodedSlibEnvelope::open(archive.as_bytes(), selection())
        .unwrap()
        .validate_graph()
        .unwrap();

    assert_eq!(graph.coordinate(), &ConeCoordinate::reserved_core());
    assert_eq!(graph.identity(), ConeIdentity::CORE);
    assert_eq!(graph.kind(), ConeKind::Library);
    assert_eq!(graph.source_form(), ConeSourceForm::Manifest);
    assert_eq!(graph.direct_dependencies(), &[expected_dependency]);
    assert_eq!(graph.target_selection(), selection());
}

#[test]
fn graph_rejects_the_reserved_single_file_dependency() {
    let members = metadata_members();
    let dependency = DependencyRecord::new(
        ConeCoordinate::reserved_single_file(),
        HirFingerprint::from_array([4; 32]),
        MirFingerprint::from_array([5; 32]),
        LirFingerprint::from_array([6; 32]),
    )
    .unwrap();
    let archive = archive(&manifest(vec![dependency], &members, Vec::new()), members);

    assert_eq!(
        DecodedSlibEnvelope::open(archive.as_bytes(), selection(),)
            .unwrap()
            .validate_graph(),
        Err(GraphValidationError::SingleFileDependency)
    );
}

#[test]
fn graph_rejects_a_direct_self_edge() {
    let members = metadata_members();
    let dependency = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        HirFingerprint::from_array([4; 32]),
        MirFingerprint::from_array([5; 32]),
        LirFingerprint::from_array([6; 32]),
    )
    .unwrap();

    let archive = archive(&manifest(vec![dependency], &members, Vec::new()), members);
    assert_eq!(
        DecodedSlibEnvelope::open(archive.as_bytes(), selection(),)
            .unwrap()
            .validate_graph(),
        Err(GraphValidationError::SelfDependency {
            cone: ConeIdentity::CORE,
        })
    );
}

#[test]
fn unknown_compile_and_link_capabilities_remain_opaque_to_graph() {
    let mut members = metadata_members();
    let cone = ConeIdentity::CORE;
    let link_capability = CapabilityId::new("org.scoop-lang.test", "link-blob", 1).unwrap();
    members.push(
        SlibMember::new(
            cone,
            MemberStableKey::ExtensionBlob {
                capability: link_capability.clone(),
                logical_key: LogicalMemberKey::new(b"opaque".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: link_capability,
                requirement: ExtensionRequirement::Link,
            },
            b"opaque link bytes".to_vec(),
        )
        .unwrap(),
    );
    let compile_section = ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "compile-manifest", 1).unwrap(),
        MemberPurposeSet::COMPILE,
        b"opaque compile bytes".to_vec(),
    )
    .unwrap();

    let archive = archive(
        &manifest(Vec::new(), &members, vec![compile_section]),
        members,
    );
    assert!(
        DecodedSlibEnvelope::open(archive.as_bytes(), selection(),)
            .unwrap()
            .validate_graph()
            .is_ok()
    );
}
