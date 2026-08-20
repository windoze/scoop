use super::*;

pub(super) fn open_graph(bytes: &[u8]) -> ValidatedGraphArtifact<'_> {
    crate::DecodedSlibEnvelope::open(bytes, selection())
        .unwrap()
        .validate_graph()
        .unwrap()
}

pub(super) fn production_manifest_section() -> ManifestSection {
    ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        crate::encoded_library_production_manifest_for_test(),
    )
    .unwrap()
}

pub(super) fn strong_section() -> MetadataSection {
    MetadataSection::new(
        MetadataLocation::Lir,
        lir_strong_production_capability(),
        MemberPurposeSet::COMPILE_AND_LINK,
        encode(&strong_production()).unwrap(),
    )
    .unwrap()
}

pub(super) fn closure_section() -> MetadataSection {
    let fixture = link_object_fixture();
    MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_for_patch_test(
            &fixture.plan,
            &fixture.builtins,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap()
}

pub(super) fn empty_hir_library_section() -> Vec<u8> {
    vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x05, 0x80]
}

pub(super) fn empty_mir_library_section() -> Vec<u8> {
    encode(
        &scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
            cone().identity(),
            scoop_mir::EntryMirBridgeBranchV1::Library,
            scoop_mir::StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
