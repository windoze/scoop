use super::{Fixture, Options};
use crate::*;
use scoop_identity::*;
use scoop_wire::{decode_canonical, encode};

use ExternalStrongShapeSubjectV1 as Subject;
const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

mod fixture;
use fixture::{ProviderFixture, consumer};
mod canonical;
mod reader;
mod relations;

#[test]
fn shape_link_replays_actual_storage_unit_and_callable_definitions_for_both_schedules() {
    for lazy in [false, true] {
        let fixture = ProviderFixture::new(lazy);
        let provider = fixture.provider();
        let support = fixture.support(true);
        let consumer = consumer();
        let unit = fixture.unit();
        let subjects = [
            Subject::StaticStorage(unit.storage()),
            Subject::StaticStorageRegistration(unit.storage()),
            Subject::StaticStorage(unit.failure_root()),
            Subject::InitializationCell(unit.unit()),
            Subject::InitializationDescriptor(unit.unit()),
            Subject::Callable(fixture.callables.records()[0].target()),
        ];
        for subject in subjects {
            let import = ExternalShapeLinkImportV1::replay(
                &provider,
                subject,
                ConeIdentity::CORE,
                &consumer,
                &support,
            )
            .unwrap();
            let physical =
                StrongShapeDefinitionRefV1::from_foundation(subject, &fixture.source.foundation)
                    .unwrap();
            assert_eq!(import.required_definition(), physical.definition());
            assert_eq!(import.expected_symbol(), physical.symbol());
            assert!(import.contract().matches_subject(subject));
            let bytes = encode(&import).unwrap();
            assert_eq!(bytes[0], 0xa5);
            let raw: DecodedExternalShapeLinkImportV1 = decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&raw).unwrap(), bytes);
            raw.validate_against(&import).unwrap();
        }
    }
}

#[test]
fn shape_link_rejects_private_support_local_use_and_consumer_defined_symbol() {
    let fixture = ProviderFixture::new(false);
    let provider = fixture.provider();
    let subject = Subject::StaticStorage(fixture.unit().storage());
    assert!(
        matches!(ExternalShapeLinkImportV1::replay(&provider, subject, ConeIdentity::CORE, &consumer(), &fixture.support(false)), Err(ShapeLinkError::SupportRelation(actual)) if actual == subject)
    );
    assert!(matches!(
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::SINGLE_FILE,
            &consumer(),
            &fixture.support(true)
        ),
        Err(ShapeLinkError::LocalImport)
    ));
    assert!(matches!(
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::CORE,
            fixture.section.canonical_definitions(),
            &fixture.support(true)
        ),
        Err(ShapeLinkError::ConsumerDefinition(_))
    ));
}

#[test]
fn shape_link_reader_rejects_definition_symbol_and_semantic_tampering() {
    let fixture = ProviderFixture::new(true);
    let import = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        Subject::InitializationDescriptor(fixture.unit().unit()),
        ConeIdentity::CORE,
        &consumer(),
        &fixture.support(true),
    )
    .unwrap();
    let bytes = encode(&import).unwrap();
    for needle in [
        import.required_definition().as_array().as_slice(),
        fixture.unit().diagnostic_path().as_bytes(),
    ] {
        let mut altered = bytes.clone();
        let index = altered
            .windows(needle.len())
            .position(|part| part == needle)
            .unwrap();
        altered[index] ^= 1;
        let raw: DecodedExternalShapeLinkImportV1 = decode_canonical(&altered).unwrap();
        assert!(raw.validate_against(&import).is_err());
    }
    let mut altered = bytes;
    let symbol = encode(&import.expected_symbol()).unwrap();
    let index = altered
        .windows(symbol.len())
        .position(|part| part == symbol)
        .unwrap();
    altered[index + symbol.len() - 1] = 2;
    let raw: DecodedExternalShapeLinkImportV1 = decode_canonical(&altered).unwrap();
    assert!(matches!(
        raw.validate_against(&import),
        Err(ShapeLinkError::Header)
    ));
}

#[test]
fn selected_initialization_import_materializes_a_typed_external_use() {
    let fixture = ProviderFixture::new(true);
    let unit = fixture.unit().unit();
    let import = ExternalShapeLinkImportV1::replay(
        &fixture.provider(),
        Subject::InitializationDescriptor(unit),
        ConeIdentity::CORE,
        &consumer(),
        &fixture.support(true),
    )
    .unwrap();
    let terminal = layout_section(
        empty_layout_exports(fixture.source.foundation.producer()),
        &[],
        Vec::new(),
    );
    let dependencies = [&terminal];
    let selected = StrongProductionDependencySelectionV2::try_new(
        ConeIdentity::CORE,
        TARGET,
        &dependencies,
        vec![import],
        &LayoutSource,
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

    assert_eq!(use_record.consumer(), ConeIdentity::CORE);
    assert_eq!(use_record.provider(), fixture.source.foundation.producer());
    assert_eq!(use_record.dependency_unit(), unit);
}

struct LayoutSource;

impl LayoutAbiSectionSourceAuthorityV1<()> for LayoutSource {
    fn validate_local_exports(&self, _: &LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }

    fn validate_physical_imports(&self, _: &[ExternalShapeLinkImportV1]) -> Result<(), ()> {
        Ok(())
    }
}

fn layout_section<'a>(
    exports: LayoutAbiExportConstituentsV1,
    dependencies: &[&'a CrossConeLayoutAbiSectionV1<'a>],
    imports: Vec<ExternalShapeLinkImportV1>,
) -> CrossConeLayoutAbiSectionV1<'a> {
    CrossConeLayoutAbiSectionV1::try_new(exports, dependencies, imports, &LayoutSource).unwrap()
}

fn layout_exports(
    layouts: CanonicalExactLayoutExportsV1,
    descriptors: CanonicalExactDescriptorExportsV1,
    dispatch: CanonicalExactDispatchExportsV1,
    callables: CanonicalExactCallableAbiExportsV1,
    foundation: &OdrFreeLirFoundation,
) -> LayoutAbiExportConstituentsV1 {
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shape_support)
        .unwrap()
}

fn empty_layout_exports(provider: ConeIdentity) -> LayoutAbiExportConstituentsV1 {
    let foundation =
        OdrFreeLirFoundation::try_new(provider, CanonicalLirFoundation::empty()).unwrap();
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
