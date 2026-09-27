use super::*;
use scoop_identity::*;

pub(super) fn replayed_provider(provider: &Provider) -> ConeProductionSectionV2 {
    let raw = provider
        .output
        .build_production_section_v2(
            provider.coordinate.clone(),
            &[],
            EntryProductionSourceV1::Library,
            &[],
        )
        .unwrap()
        .validate_layout_abi(&provider.layout_section())
        .unwrap();
    let decoded: DecodedConeProductionSectionV2 = decode_canonical(&encode(&raw).unwrap()).unwrap();
    decoded
        .replay(
            provider.coordinate.clone(),
            &[],
            TARGET,
            provider.output.foundation(),
            EntryProductionSourceV1::Library,
            &[],
            &StrongTypeReferenceDefinitionsV2::new(provider.identity, &[], &[]).unwrap(),
            &StrongInitializationDefinitionCatalogV2::new(provider.identity, &[]).unwrap(),
        )
        .unwrap()
}

pub(super) fn view<'a>(
    provider: &'a Provider,
    production: &'a ConeProductionSectionV2,
    exports: &'a LayoutAbiExportConstituentsV1,
) -> ShapeLinkProviderV1<'a> {
    ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
        foundation: provider.output.foundation(),
        production,
        ordinary: &provider.ordinary,
        layouts: exports.layouts(),
        callables: exports.callables(),
        descriptors: exports.descriptors(),
        dispatch: exports.dispatch(),
    })
    .unwrap()
}

pub(super) fn graph(
    provider: &Provider,
    relations: &[LayoutAbiDependencyV1],
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider.identity).unwrap();
    pending.register_authority(provider.exact).unwrap();
    pending
        .register_authority(provider.initialization_unit)
        .unwrap();
    let StrongCallableDefinitionOwner::Function(function) = provider.callable else {
        unreachable!()
    };
    pending.register_authority(function).unwrap();
    let exports = provider.layout_section();
    for layout in exports.layouts().records() {
        pending
            .register_authority(layout.identity().layout())
            .unwrap();
        pending.register_authority(layout.scan()).unwrap();
    }
    for dispatch in exports.dispatch().records() {
        pending.register_authority(dispatch.table()).unwrap();
    }
    for storage in provider
        .section
        .static_storage_registrations()
        .registrations()
    {
        pending
            .register_authority(storage.semantic().storage())
            .unwrap();
    }
    for relation in relations {
        assert_eq!(relation.provider(), provider.identity);
    }
    pending.finish().unwrap()
}

pub(super) fn imports<'a>(
    provider: &'a Provider,
    view: &ShapeLinkProviderV1<'a>,
    consumer: ConeIdentity,
) -> CanonicalExternalShapeLinkImportsV1 {
    let exports = provider.layout_section();
    let layout = &exports.layouts().records()[0];
    let unit = provider
        .section
        .initialization_registrations()
        .registrations()[0]
        .semantic();
    use ExternalStrongShapeSubjectV1::*;
    let subjects = [
        Callable(provider.callable),
        Layout(layout.identity().layout()),
        Scan(layout.scan()),
        TypeDescriptor(provider.exact),
        DispatchTable(exports.dispatch().records()[0].table()),
        TypeRegistration(provider.exact),
        StaticStorage(unit.storage()),
        StaticStorageRegistration(unit.storage()),
        InitializationCell(unit.unit()),
        InitializationDescriptor(unit.unit()),
    ];
    CanonicalExternalShapeLinkImportsV1::from_checked(
        subjects
            .into_iter()
            .map(|subject| ExternalShapeLinkImportV1::replay(view, subject, consumer).unwrap())
            .collect(),
    )
    .unwrap()
}
