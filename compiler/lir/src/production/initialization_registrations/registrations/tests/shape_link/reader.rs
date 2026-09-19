use super::*;

#[test]
fn shape_link_artifact_reader_uses_the_replayed_strong_v2_semantic_plans() {
    let fixture = ProviderFixture::new(true);
    let raw: DecodedStrongProductionSectionV2 =
        decode_canonical(&encode(&fixture.section).unwrap(), DecodeLimits::default()).unwrap();
    let producer = fixture.source.foundation.producer();
    let replayed = raw
        .replay(
            ConeCoordinate::reserved_single_file(),
            TARGET,
            &fixture.source.foundation,
            StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
            fixture.source.digests.clone(),
            EntryProductionSourceV1::Library,
            &[],
            CoreLirBridgeBranchV1::NotCore,
            &StrongTypeReferenceDefinitionsV2::new(producer, &[], &mut meter()).unwrap(),
            &StrongInitializationDefinitionCatalogV2::new(producer, &[], &mut meter()).unwrap(),
            &mut meter(),
        )
        .unwrap();
    let provider = ShapeLinkProviderV1::try_new(
        ShapeLinkProviderPartsV1 {
            foundation: &fixture.source.foundation,
            production: ShapeLinkProductionV1::Reader(&replayed),
            ordinary: &fixture.ordinary,
            layouts: &fixture.layouts,
            callables: &fixture.callables,
            descriptors: &fixture.descriptors,
            dispatch: &fixture.dispatch,
        },
        &mut meter(),
    )
    .unwrap();
    let subject = Subject::InitializationDescriptor(fixture.unit().unit());
    let from_reader = ExternalShapeLinkImportV1::replay(
        &provider,
        subject,
        ConeIdentity::CORE,
        &consumer(),
        &fixture.support(true),
        &mut meter(),
    )
    .unwrap();
    let from_producer = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        subject,
        ConeIdentity::CORE,
        &consumer(),
        &fixture.support(true),
        &mut meter(),
    )
    .unwrap();
    assert_eq!(
        encode(&from_reader).unwrap(),
        encode(&from_producer).unwrap()
    );
}
