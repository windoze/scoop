use super::*;
use scoop_identity::*;

pub(super) fn replayed_provider(provider: &Provider) -> ReplayedStrongProductionSectionV2 {
    let selected =
        StrongProductionDependencySelectionV2::empty(provider.identity, TARGET, &mut meter())
            .unwrap();
    let raw = provider
        .output
        .build_production_section_v2(
            provider.coordinate.clone(),
            &[],
            EntryProductionSourceV1::Library,
            &selected,
            &[],
            &mut meter(),
        )
        .unwrap()
        .validate_layout_abi(&provider.layout_section(), &mut meter())
        .unwrap()
        .into_section();
    let decoded: DecodedStrongProductionSectionV2 =
        decode_canonical(&encode(&raw).unwrap(), DecodeLimits::default()).unwrap();
    decoded
        .replay(
            provider.coordinate.clone(),
            &[],
            TARGET,
            provider.output.foundation(),
            raw.external_bridges().clone(),
            EntryProductionSourceV1::Library,
            &[],
            raw.initialization_cycle_abi().cloned().map(Box::new),
            &StrongTypeReferenceDefinitionsV2::new(provider.identity, &[], &mut meter()).unwrap(),
            &StrongInitializationDefinitionCatalogV2::new(provider.identity, &[], &mut meter())
                .unwrap(),
            &mut meter(),
        )
        .unwrap()
}

pub(super) fn view<'a>(
    provider: &'a Provider,
    production: &'a ReplayedStrongProductionSectionV2,
    exports: &'a LayoutAbiExportConstituentsV1,
) -> ShapeLinkProviderV1<'a> {
    ShapeLinkProviderV1::from_replayed(
        provider.output.foundation(),
        &provider.ordinary,
        production
            .replay_layout_exports(exports, &mut meter())
            .unwrap(),
        &mut meter(),
    )
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

pub(super) struct UnitSupport<'a>(&'a Provider);
pub(super) fn support(provider: &Provider) -> UnitSupport<'_> {
    UnitSupport(provider)
}
impl<'a> ShapeLinkSupportLookupV1<'a> for UnitSupport<'a> {
    fn support_source(
        &self,
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
        _: &mut BudgetMeter,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError> {
        assert_eq!(provider, self.0.identity);
        let unit = self
            .0
            .section
            .initialization_registrations()
            .registrations()[0]
            .semantic();
        use ExternalStrongShapeSubjectV1::*;
        Ok(match subject {
            InitializationCell(id) | InitializationDescriptor(id) if id == unit.unit() => {
                Some(ShapeLinkSupportSourceV1::Initialization { unit })
            }
            StaticStorage(id) | StaticStorageRegistration(id)
                if id == unit.storage() || id == unit.failure_root() =>
            {
                let storage = self
                    .0
                    .section
                    .static_storage_registrations()
                    .registrations()
                    .iter()
                    .find(|record| record.semantic().storage() == id)
                    .unwrap()
                    .semantic();
                Some(ShapeLinkSupportSourceV1::StaticStorage { unit, storage })
            }
            _ => None,
        })
    }
}

pub(super) fn imports<'a>(
    provider: &'a Provider,
    view: &ShapeLinkProviderV1<'a>,
    consumer: ConeIdentity,
    definitions: &StrongObjectSymbolSurfaceV1,
    support: &UnitSupport<'a>,
) -> CanonicalExternalShapeLinkImportsV1<'a> {
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
            .map(|subject| {
                ExternalShapeLinkImportV1::replay(
                    view,
                    subject,
                    consumer,
                    definitions,
                    support,
                    &mut meter(),
                )
                .unwrap()
            })
            .collect(),
        &mut meter(),
    )
    .unwrap()
}
