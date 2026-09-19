use super::{Fixture, Options};
use crate::*;
use scoop_identity::*;
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use ExternalStrongShapeSubjectV1 as Subject;
const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

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
                &mut meter(),
            )
            .unwrap();
            let physical = StrongShapeDefinitionRefV1::from_foundation(
                subject,
                &fixture.source.foundation,
                &mut meter(),
            )
            .unwrap();
            assert_eq!(import.required_definition(), physical.definition());
            assert_eq!(import.expected_symbol(), physical.symbol());
            assert!(import.contract().matches_subject(subject));
            let bytes = encode(&import).unwrap();
            assert_eq!(bytes[0], 0xa5);
            let raw: DecodedExternalShapeLinkImportV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert_eq!(encode(&raw).unwrap(), bytes);
            raw.validate_against(&import, &mut meter()).unwrap();
        }
    }
}

#[test]
fn shape_link_rejects_private_support_local_use_and_consumer_defined_symbol() {
    let fixture = ProviderFixture::new(false);
    let provider = fixture.provider();
    let subject = Subject::StaticStorage(fixture.unit().storage());
    assert!(
        matches!(ExternalShapeLinkImportV1::replay(&provider, subject, ConeIdentity::CORE, &consumer(), &fixture.support(false), &mut meter()), Err(ShapeLinkError::SupportRelation(actual)) if actual == subject)
    );
    assert!(matches!(
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::SINGLE_FILE,
            &consumer(),
            &fixture.support(true),
            &mut meter()
        ),
        Err(ShapeLinkError::LocalImport)
    ));
    assert!(matches!(
        ExternalShapeLinkImportV1::replay(
            &provider,
            subject,
            ConeIdentity::CORE,
            fixture.section.canonical_definitions(),
            &fixture.support(true),
            &mut meter()
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
        &mut meter(),
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
        let raw: DecodedExternalShapeLinkImportV1 =
            decode_canonical(&altered, DecodeLimits::default()).unwrap();
        assert!(raw.validate_against(&import, &mut meter()).is_err());
    }
    let mut altered = bytes;
    let symbol = encode(&import.expected_symbol()).unwrap();
    let index = altered
        .windows(symbol.len())
        .position(|part| part == symbol)
        .unwrap();
    altered[index + symbol.len() - 1] = 2;
    let raw: DecodedExternalShapeLinkImportV1 =
        decode_canonical(&altered, DecodeLimits::default()).unwrap();
    assert!(matches!(
        raw.validate_against(&import, &mut meter()),
        Err(ShapeLinkError::Header)
    ));
}
