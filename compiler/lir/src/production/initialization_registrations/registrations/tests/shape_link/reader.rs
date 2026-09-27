use super::*;

#[test]
fn shape_link_artifact_reader_uses_the_replayed_strong_v2_semantic_plans() {
    let fixture = ProviderFixture::new(true);
    let replayed = replay(&fixture);
    let layout_abi = layout_abi(&fixture);
    let production = replayed.validate_layout_abi(&layout_abi).unwrap();
    let provider = ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
        foundation: &fixture.source.foundation,
        production: &production,
        ordinary: &fixture.ordinary,
        layouts: layout_abi.layouts(),
        callables: layout_abi.callables(),
        descriptors: layout_abi.descriptors(),
        dispatch: layout_abi.dispatch(),
    })
    .unwrap();
    let subject = Subject::InitializationDescriptor(fixture.unit().unit());
    let from_reader =
        ExternalShapeLinkImportV1::replay(&provider, subject, ConeIdentity::CORE).unwrap();
    let from_producer =
        ExternalShapeLinkImportV1::replay(&fixture.provider(), subject, ConeIdentity::CORE)
            .unwrap();
    assert_eq!(
        encode(&from_reader).unwrap(),
        encode(&from_producer).unwrap()
    );
}

fn replay(fixture: &ProviderFixture) -> ConeProductionSectionV2 {
    let raw: DecodedConeProductionSectionV2 =
        decode_canonical(&encode(&fixture.section).unwrap()).unwrap();
    let producer = fixture.source.foundation.producer();
    raw.replay(
        ConeCoordinate::reserved_single_file(),
        &[],
        TARGET,
        &fixture.source.foundation,
        EntryProductionSourceV1::Library,
        &[],
        &StrongTypeReferenceDefinitionsV2::new(producer, &[], &[]).unwrap(),
        &StrongInitializationDefinitionCatalogV2::new(producer, &[]).unwrap(),
    )
    .unwrap()
}

fn layout_abi(fixture: &ProviderFixture) -> CrossConeLayoutAbiSectionV1<'static> {
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(TARGET, &fixture.source.foundation, Vec::new())
            .unwrap();
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &fixture.layouts,
        &fixture.descriptors,
        &fixture.source.foundation,
    )
    .unwrap();
    let exports = LayoutAbiExportConstituentsV1::try_new(
        fixture.layouts.clone(),
        fixture.descriptors.clone(),
        fixture.dispatch.clone(),
        callables,
        shape_support,
        CrossConeLirBridgeSectionV1::try_new(&fixture.source.foundation, Vec::new(), Vec::new())
            .unwrap(),
    )
    .unwrap();
    CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &[]).unwrap()
}
