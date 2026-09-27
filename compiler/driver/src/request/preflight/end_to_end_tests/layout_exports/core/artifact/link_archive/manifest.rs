use super::*;

pub(in super::super) fn rewrite_production(
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    change: impl Fn(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let first = entries.next().unwrap().unwrap();
    assert_eq!(first.name(), b"manifest.cbor");
    let manifest =
        scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(first.data(bytes).unwrap())
            .unwrap()
            .validate(artifact.target_selection())
            .unwrap();
    let members = manifest
        .members()
        .iter()
        .map(|record| {
            let data = entries.next().unwrap().unwrap().data(bytes).unwrap();
            slib::SlibMember::new(
                manifest.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                data.to_vec(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(entries.next().is_none());
    let mut changed = 0;
    let sections = manifest
        .sections()
        .iter()
        .map(|section| {
            let payload =
                if section.capability() == &slib::manifest_single_cone_production_capability() {
                    changed += 1;
                    change(section.payload())
                } else {
                    section.payload().to_vec()
                };
            slib::ManifestSection::new(
                section.capability().clone(),
                section.required_for(),
                payload,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(changed, 1);
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("layout-runtime-negative").unwrap(),
        manifest.compatibility().clone(),
        manifest.cone().clone(),
        manifest.direct_dependencies().to_vec(),
        &members,
        manifest.semantic_fingerprints(),
        sections,
    )
    .unwrap();
    slib::CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .into_bytes()
}
