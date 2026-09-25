use super::*;

mod projection;

pub(super) fn replace(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    failure: Failure,
) -> Vec<u8> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let first = entries.next().unwrap().unwrap();
    assert_eq!(first.name(), b"manifest.cbor");
    let manifest = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        first.data(bytes).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
    .validate(artifact.target_selection(), &mut meter())
    .unwrap();
    let mut members = Vec::new();
    let mut changed = 0;
    for record in manifest.members() {
        let data = entries.next().unwrap().unwrap().data(bytes).unwrap();
        if matches!(failure, Failure::MissingObject)
            && changed == 0
            && matches!(record.role(), slib::SlibMemberRole::LinkObject { .. })
        {
            changed += 1;
            continue;
        }
        let payload = if !matches!(failure, Failure::MissingObject)
            && matches!(record.role(), slib::SlibMemberRole::LirMetadata)
        {
            let decoded = slib::DecodedMetadataEnvelope::decode(
                data,
                slib::MetadataLocation::Lir,
                DecodeLimits::default(),
            )
            .unwrap();
            let sections = decoded
                .sections()
                .iter()
                .map(|section| {
                    let payload =
                        if section.capability() == &slib::lir_link_identity_closure_capability() {
                            changed += 1;
                            projection::mutate(section.payload(), failure)
                        } else {
                            section.payload().to_vec()
                        };
                    slib::MetadataSection::new(
                        slib::MetadataLocation::Lir,
                        section.capability().clone(),
                        section.required_for(),
                        payload,
                    )
                    .unwrap()
                })
                .collect();
            encode(&slib::MetadataEnvelope::new(slib::MetadataLocation::Lir, sections).unwrap())
                .unwrap()
        } else {
            data.to_vec()
        };
        members.push(
            slib::SlibMember::new(
                manifest.cone().identity(),
                record.stable_key().clone(),
                record.role().clone(),
                payload,
            )
            .unwrap(),
        );
    }
    assert!(entries.next().is_none());
    assert_eq!(changed, 1);
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("layout-materialization-negative").unwrap(),
        manifest.compatibility().clone(),
        manifest.cone().clone(),
        manifest.direct_dependencies().to_vec(),
        &members,
        manifest.semantic_fingerprints(),
        manifest.sections().to_vec(),
    )
    .unwrap();
    slib::CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .into_bytes()
}
