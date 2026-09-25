//! A hash-valid archive must still reject a different Link-only provider.

use super::*;
use scoop_slib as slib;

pub(super) fn check(
    provider: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let expected_provider = layout.selected().physical_imports().records()[0].provider();
    let bytes = wrong_link_provider(artifact, expected_provider);
    let link =
        DecodedSlibEnvelope::open(&bytes, DecodeLimits::default(), artifact.target_selection())
            .unwrap()
            .validate_graph()
            .unwrap()
            .decode_cross_cone_layout_link_sections()
            .unwrap();
    let shared = read_sections(
        open_link(provider).into_shared_sections().unwrap(),
        link.into_shared_sections().unwrap(),
    );
    let error = shared.with_replayed_physical_imports(|_| ()).unwrap_err();
    assert_eq!(error.provider, layout.provider());
    assert!(matches!(
        *error.source,
        slib::SharedLirPhysicalError::LinkImports(error)
            if matches!(*error, slib::LayoutLinkClosureError::SemanticImports(_))
    ));
}

fn wrong_link_provider(
    artifact: &slib::AssembledCrossConeLayoutStrongArtifactV1,
    provider: ConeIdentity,
) -> Vec<u8> {
    let bytes = artifact.as_bytes();
    let archive = object::read::archive::ArchiveFile::parse(bytes).unwrap();
    let mut entries = archive.members();
    let entry = entries.next().unwrap().unwrap();
    assert_eq!(entry.name(), b"manifest.cbor");
    let manifest = scoop_wire::decode_canonical::<slib::DecodedBootstrapManifest>(
        entry.data(bytes).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
    .validate(artifact.target_selection(), &mut meter())
    .unwrap();
    let mut members = Vec::new();
    let mut replacements = 0;
    for record in manifest.members() {
        let data = entries.next().unwrap().unwrap().data(bytes).unwrap();
        let payload = if matches!(record.role(), slib::SlibMemberRole::LirMetadata) {
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
                    let mut payload = section.payload().to_vec();
                    if section.capability()
                        == &slib::lir_cross_cone_layout_link_closure_capability()
                    {
                        let offset = payload
                            .windows(32)
                            .position(|part| part == provider.as_array())
                            .unwrap();
                        payload[offset] ^= 1;
                        replacements += 1;
                    }
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
    assert_eq!(replacements, 1);
    let manifest = slib::BootstrapManifest::new(
        slib::ProducerRecord::new("layout-link-negative").unwrap(),
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
