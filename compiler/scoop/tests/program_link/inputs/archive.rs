//! Repack actual compiler outputs to exercise the artifact input boundary.
use super::*;
use scoop_slib::{
    BootstrapManifest, CanonicalSlibArchive, DecodedBootstrapManifest, ManifestSection,
    ProducerRecord, SlibMember,
};

pub(super) fn read(path: &Path) -> (BootstrapManifest, Vec<SlibMember>) {
    let bytes = std::fs::read(path).unwrap();
    let archive = object::read::archive::ArchiveFile::parse(bytes.as_slice()).unwrap();
    let mut physical = archive.members();
    let first = physical.next().unwrap().unwrap();
    assert_eq!(first.name(), b"manifest.cbor");
    let manifest = scoop_wire::decode_canonical::<DecodedBootstrapManifest>(
        first.data(bytes.as_slice()).unwrap(),
    )
    .unwrap()
    .validate(scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1)
    .unwrap();
    let members = manifest
        .members()
        .iter()
        .map(|record| {
            let member = physical.next().unwrap().unwrap();
            SlibMember::new(
                manifest.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                member.data(bytes.as_slice()).unwrap().to_vec(),
            )
            .unwrap()
        })
        .collect();
    assert!(physical.next().is_none());
    (manifest, members)
}

pub(super) fn write(
    path: &Path,
    original: &BootstrapManifest,
    members: Vec<SlibMember>,
    sections: Vec<ManifestSection>,
) {
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("m23-program-link-repack").unwrap(),
        original.compatibility().clone(),
        original.cone().clone(),
        original.direct_dependencies().to_vec(),
        &members,
        original.semantic_fingerprints(),
        sections,
    )
    .unwrap();
    let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();
    std::fs::write(path, archive.as_bytes()).unwrap();
}
