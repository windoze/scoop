use super::*;
use crate::StrongRegistrationProductionValidationError as Error;

mod local_catalog;
mod surface;

fn definition(fixture: &Fixture) -> Definition {
    Definition::from_foundation(fixture.unit, &fixture.foundation, &fixture.identities).unwrap()
}

fn decoded(plans: &Plans) -> Vec<DecodedStrongInitializationUnitRegistrationPlanV1> {
    plans
        .registrations()
        .iter()
        .map(|plan| decode_canonical(&encode(plan).unwrap()).unwrap())
        .collect()
}

fn validate(
    fixture: &Fixture,
    records: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    definitions: &Catalog,
) -> Result<Semantics, Error> {
    crate::validate_initialization_registration_constituents_v2(
        records,
        LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.identities,
        fixture.semantics.static_storages().clone(),
        definitions,
        &fixture.digests,
    )
}

#[test]
fn complete_reader_replays_both_schedules_and_keeps_foreign_dependencies_typed() {
    for lazy in [false, true] {
        let consumer = Fixture::new(Options {
            lazy,
            ..Options::default()
        });
        let provider = Fixture::with_source(Options::default(), ConeIdentity::CORE, "provider");
        let definitions =
            Catalog::new(ConeIdentity::SINGLE_FILE, &[definition(&provider)]).unwrap();
        let reference = dependency(ConeIdentity::SINGLE_FILE, consumer.unit, &provider);
        let original = build(&consumer, vec![reference]).unwrap();
        let semantics = validate(&consumer, decoded(&original), &definitions).unwrap();
        assert_eq!(semantics.units()[0].dependencies(), &[reference]);
        assert!(matches!(
            semantics.units()[0].dependencies()[0].kind(),
            Kind::DependencyExternalUnit {
                provider: ConeIdentity::CORE,
                ..
            }
        ));
        let replayed = Plans::new(
            &consumer.foundation,
            &consumer.identities,
            &semantics,
            &consumer.digests,
        )
        .unwrap();
        assert_eq!(
            encode(&original.registrations()[0]).unwrap(),
            encode(&replayed.registrations()[0]).unwrap()
        );
    }
}

#[test]
fn local_dependency_definition_must_belong_to_the_actual_local_tables() {
    let consumer = Fixture::new(Options::default());
    let absent_local = Fixture::with_source(
        Options::default(),
        ConeIdentity::SINGLE_FILE,
        "absent_local",
    );
    let definitions =
        Catalog::new(ConeIdentity::SINGLE_FILE, &[definition(&absent_local)]).unwrap();
    let reference = dependency(ConeIdentity::SINGLE_FILE, consumer.unit, &absent_local);
    let plans = build(&consumer, vec![reference]).unwrap();
    assert!(matches!(
        validate(&consumer, decoded(&plans), &definitions),
        Err(Error::InitializationDefinition(
            crate::InitializationDefinitionResolutionErrorV2::MissingRegistrationIdentity(_)
        ))
    ));
}

#[test]
fn missing_dependencies_and_a_catalog_from_another_consumer_are_rejected() {
    let consumer = Fixture::new(Options::default());
    let provider = Fixture::with_source(Options::default(), ConeIdentity::CORE, "provider");
    let reference = dependency(ConeIdentity::SINGLE_FILE, consumer.unit, &provider);
    let plans = build(&consumer, vec![reference]).unwrap();
    let empty = Catalog::new(ConeIdentity::SINGLE_FILE, &[]).unwrap();
    assert!(matches!(
        validate(&consumer, decoded(&plans), &empty),
        Err(Error::InitializationDependency(
            crate::InitializationDependencyResolutionError::UnknownUnit(_)
        ))
    ));
    let other = Catalog::new(ConeIdentity::CORE, &[]).unwrap();
    let no_dependencies = build(&consumer, Vec::new()).unwrap();
    assert!(matches!(
        validate(&consumer, decoded(&no_dependencies), &other),
        Err(Error::Semantic {
            field: "producer",
            ..
        })
    ));
    validate(&consumer, decoded(&no_dependencies), &empty).unwrap();
}

#[test]
fn complete_reader_recomputes_every_physical_field_and_table_coverage() {
    let fixture = Fixture::new(Options::default());
    let definitions = Catalog::new(ConeIdentity::SINGLE_FILE, &[]).unwrap();
    let plans = build(&fixture, Vec::new()).unwrap();
    assert!(matches!(
        validate(&fixture, Vec::new(), &definitions),
        Err(Error::TableLength { .. })
    ));
    let mut bytes = encode(&plans.registrations()[0]).unwrap();
    let plan = plans.registrations()[0].registration_definition_plan();
    let position = bytes
        .windows(32)
        .position(|bytes| bytes == plan.as_array())
        .unwrap();
    bytes[position] ^= 1;
    let altered = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        validate(&fixture, vec![altered], &definitions),
        Err(Error::EntryMismatch { index: 0, .. })
    ));
}
