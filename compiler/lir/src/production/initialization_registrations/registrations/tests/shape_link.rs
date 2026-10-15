use super::{Fixture, Options};
use crate::*;
use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

use ExternalStrongShapeSubjectV1 as Subject;
const TARGET: LirTargetProfile = crate::target_fixtures::DARWIN_AARCH64;

mod fixture;
use fixture::ProviderFixture;
mod canonical;

#[test]
fn shape_link_replays_actual_storage_unit_and_callable_definitions_for_both_schedules() {
    for lazy in [false, true] {
        let fixture = ProviderFixture::new(lazy);
        let provider = fixture.provider();
        let unit = fixture.unit();
        let subjects = [
            Subject::StaticStorage(unit.storage()),
            Subject::StaticStorageRegistration(unit.storage()),
            Subject::StaticStorage(unit.failure_root()),
            Subject::InitializationCell(unit.unit()),
            Subject::InitializationRegistration(unit.unit()),
            Subject::Callable(fixture.callables.records()[0].target()),
        ];
        for subject in subjects {
            let import =
                ExternalShapeLinkImportV1::replay(&provider, subject, ConeIdentity::CORE).unwrap();
            let physical =
                StrongShapeDefinitionRefV1::from_foundation(subject, &fixture.source.foundation)
                    .unwrap();
            assert_eq!(import.required_definition(), physical.definition());
            assert_eq!(import.expected_symbol(), physical.symbol());
            assert!(import.contract().matches_subject(subject));
            let bytes = encode(&import).unwrap();
            assert_eq!(bytes[0], 0xa4);
            let raw: DecodedExternalShapeLinkImportV1 = decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&raw).unwrap(), bytes);
        }
    }
}

#[test]
fn shape_link_rejects_same_provider_imports() {
    let fixture = ProviderFixture::new(false);
    let provider = fixture.provider();
    let subject = Subject::StaticStorage(fixture.unit().storage());
    assert!(matches!(
        ExternalShapeLinkImportV1::replay(&provider, subject, provider.provider()),
        Err(ShapeLinkError::LocalImport)
    ));
}

#[test]
fn selected_initialization_import_materializes_a_typed_external_use() {
    let fixture = ProviderFixture::new(true);
    let unit = fixture.unit().unit();
    let import = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        Subject::InitializationRegistration(unit),
        ConeIdentity::CORE,
    )
    .unwrap();
    let terminal = layout_section(
        empty_layout_exports(fixture.source.foundation.producer()),
        &[],
        Vec::new(),
    );
    let dependencies = [terminal.exports()];
    let selected = StrongProductionDependencySelectionV2::try_new(
        ConeIdentity::CORE,
        TARGET,
        &dependencies,
        vec![import],
        &[],
    )
    .unwrap();
    let definition = StrongInitializationUnitDefinitionRefV2::from_registrations(
        fixture
            .section
            .registration_production()
            .initialization_units(),
        unit,
    )
    .unwrap();
    let use_record =
        StrongExternalInitializationUseV2::try_new(unit, definition, &selected).unwrap();

    assert_eq!(use_record.provider(), fixture.source.foundation.producer());
    assert_eq!(use_record.dependency_unit(), unit);
}

fn layout_section<'a>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    imports: Vec<ExternalShapeLinkImportV1>,
) -> CrossConeLayoutAbiSectionV1<'a> {
    CrossConeLayoutAbiSectionV1::try_new(exports, dependencies, imports, &[]).unwrap()
}

fn layout_exports(
    layouts: CanonicalExactLayoutExportsV1,
    descriptors: CanonicalExactDescriptorExportsV1,
    dispatch: CanonicalExactDispatchExportsV1,
    callables: CanonicalExactCallableAbiExportsV1,
    foundation: &ConeLirFoundation,
) -> LayoutAbiExportConstituentsV1 {
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        dispatch,
        callables,
        shape_support,
        crate::CrossConeLirBridgeSectionV1::try_new(foundation, Vec::new(), Vec::new()).unwrap(),
    )
    .unwrap()
}

fn empty_layout_exports(provider: ConeIdentity) -> LayoutAbiExportConstituentsV1 {
    let foundation = ConeLirFoundation::try_new(provider, CanonicalLirFoundation::empty()).unwrap();
    let layouts = CanonicalExactLayoutExportsV1::try_new(TARGET, &foundation, Vec::new()).unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(TARGET, &foundation, Vec::new()).unwrap();
    layout_exports(
        layouts,
        descriptors,
        CanonicalExactDispatchExportsV1::try_new(TARGET, &foundation, Vec::new()).unwrap(),
        CanonicalExactCallableAbiExportsV1::try_new(TARGET, &foundation, Vec::new()).unwrap(),
        &foundation,
    )
}
