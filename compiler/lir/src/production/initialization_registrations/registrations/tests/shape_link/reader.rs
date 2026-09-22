use super::*;

struct Source;

impl LayoutAbiSectionSourceAuthorityV1<()> for Source {
    fn validate_local_exports(
        &self,
        _: &LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(
        &self,
        imports: &[ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        if imports.is_empty() { Ok(()) } else { Err(()) }
    }
}

#[test]
fn shape_link_artifact_reader_uses_the_replayed_strong_v2_semantic_plans() {
    let fixture = ProviderFixture::new(true);
    let replayed = replay(&fixture);
    let layout_abi = layout_abi(&fixture);
    let production = replayed
        .validate_layout_abi(&layout_abi, &mut meter())
        .unwrap();
    let provider = ShapeLinkProviderV1::try_new(
        ShapeLinkProviderPartsV1 {
            foundation: &fixture.source.foundation,
            production: ShapeLinkProductionV1::Reader(&production),
            ordinary: &fixture.ordinary,
            layouts: layout_abi.layouts(),
            callables: layout_abi.callables(),
            descriptors: layout_abi.descriptors(),
            dispatch: layout_abi.dispatch(),
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

#[test]
fn shape_link_reader_rejects_tables_outside_the_validated_layout_closure() {
    let fixture = ProviderFixture::new(true);
    let layout_abi = layout_abi(&fixture);
    let production = replay(&fixture)
        .validate_layout_abi(&layout_abi, &mut meter())
        .unwrap();
    assert!(matches!(
        ShapeLinkProviderV1::try_new(
            ShapeLinkProviderPartsV1 {
                foundation: &fixture.source.foundation,
                production: ShapeLinkProductionV1::Reader(&production),
                ordinary: &fixture.ordinary,
                layouts: layout_abi.layouts(),
                callables: &fixture.callables,
                descriptors: layout_abi.descriptors(),
                dispatch: layout_abi.dispatch(),
            },
            &mut meter(),
        ),
        Err(ShapeLinkError::Provider)
    ));
}

fn replay(fixture: &ProviderFixture) -> ReplayedStrongProductionSectionV2 {
    let raw: DecodedStrongProductionSectionV2 =
        decode_canonical(&encode(&fixture.section).unwrap(), DecodeLimits::default()).unwrap();
    let producer = fixture.source.foundation.producer();
    raw.replay(
        ConeCoordinate::reserved_single_file(),
        TARGET,
        &fixture.source.foundation,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
        fixture.source.digests.clone(),
        EntryProductionSourceV1::Library,
        &[],
        None,
        &StrongTypeReferenceDefinitionsV2::new(producer, &[], &mut meter()).unwrap(),
        &StrongInitializationDefinitionCatalogV2::new(producer, &[], &mut meter()).unwrap(),
        &mut meter(),
    )
    .unwrap()
}

fn layout_abi(fixture: &ProviderFixture) -> CrossConeLayoutAbiSectionV1<'static> {
    let callables = CanonicalExactCallableAbiExportsV1::try_new(
        TARGET,
        &fixture.source.foundation,
        Vec::new(),
        &mut meter(),
    )
    .unwrap();
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &fixture.layouts,
        &fixture.descriptors,
        &fixture.source.foundation,
        &mut meter(),
    )
    .unwrap();
    let exports = LayoutAbiExportConstituentsV1::try_new(
        fixture.layouts.clone(),
        fixture.descriptors.clone(),
        fixture.dispatch.clone(),
        callables,
        shape_support,
    )
    .unwrap();
    CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &Source, &mut meter()).unwrap()
}
