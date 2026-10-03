//! Container fixtures for manifest, discovery, and cache tests.
//! Opaque IR members intentionally remain outside compiler/Link validation.

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{
    ArtifactCapabilityProfile, BootstrapManifest, CanonicalSlibArchive, CodeFingerprint,
    CompatibilityRecord, ConeRecord, DependencyRecord, FingerprintAvailability, MemberStableKey,
    ProducerRecord, RuntimeImageFingerprint, SemanticFingerprintRecord, SlibMember, SlibMemberRole,
};

pub(crate) fn manifest_archive(
    profile: ArtifactCapabilityProfile,
    cone: ConeRecord,
    producer: &str,
    dependencies: Vec<DependencyRecord>,
) -> CanonicalSlibArchive {
    let members = [
        (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata),
        (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata),
        (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata),
    ]
    .into_iter()
    .map(|(key, role)| {
        SlibMember::new(cone.identity(), key, role, b"opaque metadata".to_vec()).unwrap()
    })
    .collect::<Vec<_>>();
    let compatibility = CompatibilityRecord::new(
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        profile,
    )
    .unwrap();
    let compile = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &[],
        &[],
        &[],
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_digests(
        compile.hir(),
        compile.mir(),
        compile.lir(),
        FingerprintAvailability::Available(CodeFingerprint::from_array([4; 32])),
        FingerprintAvailability::Available(RuntimeImageFingerprint::from_array([5; 32])),
    );
    let manifest = BootstrapManifest::new(
        ProducerRecord::new(producer).unwrap(),
        compatibility,
        cone,
        dependencies,
        &members,
        semantic,
        Vec::new(),
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap()
}
