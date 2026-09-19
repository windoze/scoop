use super::*;
use crate::{
    DecodedStrongInitializationUnitRegistrationPlanV1,
    StrongInitializationDefinitionCatalogV2 as Catalog,
    StrongInitializationDependencyKindV2 as Kind,
    StrongInitializationDependencyRefV2 as Dependency,
    StrongInitializationUnitDefinitionRefV2 as Definition,
    StrongInitializationUnitRegistrationPlanSetV2 as Plans,
    StrongInitializationUnitSemanticPlanSetV2 as Semantics,
    StrongInitializationUnitSemanticPlanV2 as Semantic,
};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

fn build(
    fixture: &Fixture,
    dependencies: Vec<Dependency>,
) -> Result<Plans, StrongInitializationUnitRegistrationPlanBuildError> {
    let semantic = &fixture.semantics.units()[0];
    let semantics = Semantics::from_artifact(
        fixture.semantics.static_storages().clone(),
        vec![Semantic::from_artifact(
            semantic.unit(),
            semantic.diagnostic_path().to_owned(),
            semantic.schedule(),
            semantic.storage(),
            semantic.failure_root(),
            semantic.initializer(),
            semantic.ensure(),
            dependencies,
        )],
    );
    Plans::new(
        &fixture.foundation,
        &fixture.identities,
        &semantics,
        &fixture.digests,
    )
}

fn dependency(
    consumer: ConeIdentity,
    unit: PersistentInitializationUnitId,
    provider: &Fixture,
) -> Dependency {
    let plans = provider.build().unwrap();
    let definition = Definition::from_registrations(&plans, provider.unit).unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let catalog = Catalog::new(consumer, &[definition], &mut meter).unwrap();
    let id = decode_canonical(&encode(&provider.unit).unwrap(), DecodeLimits::default()).unwrap();
    catalog
        .resolve(unit, &[id], &mut meter)
        .unwrap()
        .references()[0]
}

#[test]
fn both_schedules_keep_all_twenty_eight_registration_fields_unchanged() {
    for lazy in [false, true] {
        let fixture = Fixture::new(Options {
            lazy,
            ..Options::default()
        });
        let old = fixture.build().unwrap();
        let current = build(&fixture, Vec::new()).unwrap();
        let bytes = encode(&current.registrations()[0]).unwrap();
        assert_eq!(bytes, encode(&old.registrations()[0]).unwrap());
        assert_eq!(&bytes[..2], &[0xb8, 28]);
        let decoded: DecodedStrongInitializationUnitRegistrationPlanV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            Definition::from_registrations(&current, fixture.unit),
            Definition::from_registrations(&old, fixture.unit)
        );
    }
}

#[test]
fn complete_registration_keeps_local_and_foreign_definition_refs_in_memory_only() {
    for producer in [ConeIdentity::SINGLE_FILE, ConeIdentity::CORE] {
        let mut consumer = Fixture::new(Options::default());
        let provider = Fixture::with_source(Options::default(), producer, "provider");
        let reference = dependency(consumer.foundation.producer(), consumer.unit, &provider);
        let current = build(&consumer, vec![reference]).unwrap();
        let retained = &current.registrations()[0].semantic().dependencies()[0];
        match retained.kind() {
            Kind::LocalUnit(definition) => {
                assert_eq!(definition.provider(), consumer.foundation.producer())
            }
            Kind::DependencyExternalUnit { provider, unit_ref } => {
                assert_eq!(provider, ConeIdentity::CORE);
                assert_eq!(unit_ref.provider(), provider);
            }
        }
        assert_eq!(retained.definition(), reference.definition());
        consumer.semantics.units[0].dependencies = vec![provider.unit];
        let old = consumer.build().unwrap();
        assert_eq!(
            encode(&current.registrations()[0]).unwrap(),
            encode(&old.registrations()[0]).unwrap()
        );
    }
}

#[test]
fn reusing_a_reference_in_another_consumer_cannot_change_its_provider_role() {
    let consumer = Fixture::new(Options::default());
    let provider = Fixture::with_source(Options::default(), ConeIdentity::SINGLE_FILE, "provider");
    let foreign = dependency(ConeIdentity::CORE, consumer.unit, &provider);
    assert!(matches!(
        build(&consumer, vec![foreign]),
        Err(StrongInitializationUnitRegistrationPlanBuildError::DependencyProviderRole { .. })
    ));
}

#[test]
fn both_versions_reject_duplicate_and_self_dependencies_before_registration() {
    let mut consumer = Fixture::new(Options::default());
    let provider = Fixture::with_source(Options::default(), ConeIdentity::CORE, "provider");
    let reference = dependency(consumer.foundation.producer(), consumer.unit, &provider);
    assert!(matches!(
        build(&consumer, vec![reference, reference]),
        Err(StrongInitializationUnitRegistrationPlanBuildError::DependencyOrder { index: 1, .. })
    ));
    consumer.semantics.units[0].dependencies = vec![provider.unit, provider.unit];
    assert!(matches!(
        consumer.build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::DependencyOrder { index: 1, .. })
    ));
    consumer.semantics.units[0].dependencies = vec![consumer.unit];
    assert_eq!(
        consumer.build().unwrap_err(),
        StrongInitializationUnitRegistrationPlanBuildError::SelfDependency(consumer.unit)
    );
}

#[test]
fn version_two_still_requires_complete_descriptor_definition_and_digest() {
    for options in [
        Options {
            omit_descriptor_primary: true,
            ..Options::default()
        },
        Options {
            omit_descriptor_input: true,
            ..Options::default()
        },
    ] {
        let fixture = Fixture::new(options);
        assert_eq!(
            build(&fixture, Vec::new()).unwrap_err(),
            fixture.build().unwrap_err()
        );
    }
}
