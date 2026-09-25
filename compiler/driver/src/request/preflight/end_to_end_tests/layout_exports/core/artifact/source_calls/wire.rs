use super::*;

mod manifest;

pub(super) fn replace_public(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    mut public: hir::CrossConeHirInterfaceSectionV1,
) -> Vec<u8> {
    let replacement = encode(&public.index_for_wire().unwrap()).unwrap();
    replace_hir_section(
        artifact,
        &slib::hir_cross_cone_interface_capability(),
        replacement,
    )
}

pub(super) fn replace_types(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    types: &hir::CrossConeTypeSemanticsSectionV1,
) -> Vec<u8> {
    let replacement = encode(&types.index_for_wire().unwrap()).unwrap();
    replace_hir_section(
        artifact,
        &slib::hir_cross_cone_type_semantics_capability(),
        replacement,
    )
}

fn replace_hir_section(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    capability: &scoop_identity::CapabilityId,
    replacement: Vec<u8>,
) -> Vec<u8> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let entry = entries.next().unwrap().unwrap();
    assert_eq!(entry.name(), b"manifest.cbor");
    let manifest =
        scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(entry.data(bytes).unwrap())
            .unwrap()
            .validate(artifact.target_selection())
            .unwrap();
    let mut members = Vec::new();
    let mut replaced = 0;
    let mut hir_fingerprint = None;
    for record in manifest.members() {
        let data = entries.next().unwrap().unwrap().data(bytes).unwrap();
        let payload = if matches!(record.role(), slib::SlibMemberRole::HirMetadata) {
            let decoded =
                slib::DecodedMetadataEnvelope::decode(data, slib::MetadataLocation::Hir).unwrap();
            let sections = decoded
                .sections()
                .iter()
                .map(|section| {
                    let payload = if section.capability() == capability {
                        replaced += 1;
                        replacement.clone()
                    } else {
                        section.payload().to_vec()
                    };
                    slib::MetadataSection::new(
                        slib::MetadataLocation::Hir,
                        section.capability().clone(),
                        section.required_for(),
                        payload,
                    )
                    .unwrap()
                })
                .collect();
            let envelope =
                slib::MetadataEnvelope::new(slib::MetadataLocation::Hir, sections).unwrap();
            hir_fingerprint = Some(
                slib::SemanticFingerprintRecord::from_metadata_sections(
                    manifest.compatibility(),
                    manifest.direct_dependencies(),
                    envelope.sections(),
                    &[],
                    &[],
                )
                .unwrap()
                .hir(),
            );
            encode(&envelope).unwrap()
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
    assert_eq!(replaced, 1);
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("source-call-negative").unwrap(),
        manifest.compatibility().clone(),
        manifest.cone().clone(),
        manifest.direct_dependencies().to_vec(),
        &members,
        manifest.semantic_fingerprints(),
        manifest.sections().to_vec(),
    )
    .unwrap();
    let manifest = manifest::with_hir_fingerprint(&manifest, hir_fingerprint.unwrap());
    scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(&manifest)
        .unwrap()
        .validate(artifact.target_selection())
        .unwrap();
    slib::CanonicalSlibArchive::write(&manifest, members)
        .unwrap()
        .into_bytes()
}
