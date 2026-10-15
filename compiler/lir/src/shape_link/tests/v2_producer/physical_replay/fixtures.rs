use super::*;
use scoop_identity::*;

pub(super) fn produced_provider(provider: &Provider) -> ConeProductionSectionV2 {
    provider
        .output
        .build_production_section_v2(
            provider.coordinate.clone(),
            &[],
            EntryProductionSourceV1::Library,
            &[],
        )
        .unwrap()
        .validate_layout_abi(&provider.layout_section())
        .unwrap()
}

pub(super) fn view<'a>(
    provider: &'a Provider,
    production: &'a ConeProductionSectionV2,
    exports: &'a LayoutAbiExportConstituentsV1,
) -> ShapeLinkProviderV1<'a> {
    ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
        foundation: provider.output.foundation(),
        definitions: production.canonical_definitions(),
        support: ShapeLinkSupportV1::Produced {
            static_storages: production.static_storage_registrations(),
            initialization: production.initialization_registrations(),
        },
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
    pending_graph(provider, relations).finish().unwrap()
}

fn pending_graph(
    provider: &Provider,
    relations: &[LayoutAbiDependencyV1],
) -> PendingIdentityValidation {
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
    pending.register_authority(provider.callable_body).unwrap();
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
    for plan in provider.output.foundation().definition_plans() {
        pending.register_authority(plan.id()).unwrap();
    }
    for relation in relations {
        assert_eq!(relation.provider(), provider.identity);
    }
    pending
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
        Callable(scoop_identity::CallableDefinitionOwner::Strong(
            provider.callable,
        )),
        Layout(layout.identity().layout()),
        Scan(layout.scan()),
        TypeDescriptor(provider.exact),
        DispatchTable(exports.dispatch().records()[0].table()),
        TypeRegistration(provider.exact),
        StaticStorage(unit.storage()),
        StaticStorageRegistration(unit.storage()),
        InitializationCell(unit.unit()),
        InitializationRegistration(unit.unit()),
    ];
    CanonicalExternalShapeLinkImportsV1::from_checked(
        subjects
            .into_iter()
            .map(|subject| ExternalShapeLinkImportV1::replay(view, subject, consumer).unwrap())
            .collect(),
    )
    .unwrap()
}
