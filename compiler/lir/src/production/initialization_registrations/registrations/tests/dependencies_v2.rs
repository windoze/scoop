use super::*;
use crate::{
    InitializationDependencyResolutionError as Error,
    StrongInitializationDefinitionCatalogV2 as Catalog,
    StrongInitializationDependencyKindV2 as Kind,
    StrongInitializationUnitDefinitionRefV2 as Definition,
};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decoded(
    unit: PersistentInitializationUnitId,
) -> DecodedPersistentId<PersistentInitializationUnitId> {
    decode_canonical(&encode(&unit).unwrap(), DecodeLimits::default()).unwrap()
}
fn source_unit() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId::from_key(&InitializationUnitKey::TopLevelProperty(property(
        "source",
    )))
    .unwrap()
}

#[test]
fn dependency_definitions_retain_provider_and_all_three_physical_relations() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let definition = Definition::from_registrations(&plans, fixture.unit).unwrap();
    let plan = &plans.registrations()[0];
    assert_eq!(definition.provider(), plans.producer());
    assert_eq!(definition.unit(), fixture.unit);
    assert_eq!(
        definition.descriptor().plan(),
        plan.descriptor_definition_plan()
    );
    assert_eq!(
        definition.descriptor().primary(),
        plan.descriptor_primary_atom()
    );
    assert_eq!(definition.descriptor().symbol(), plan.descriptor_symbol());
    assert_eq!(definition.cell().plan(), plan.cell_definition_plan());
    assert_eq!(definition.cell().primary(), plan.cell_primary_atom());
    assert_eq!(definition.cell().symbol(), plan.cell_symbol());
    assert_eq!(
        definition.registration().plan(),
        plan.registration_definition_plan()
    );
    assert_eq!(
        definition.registration().primary(),
        plan.registration_primary_atom()
    );
    assert_eq!(
        definition.registration().symbol(),
        plan.registration_symbol()
    );
    assert_eq!(
        definition.registration_fingerprint(),
        plan.registration_fingerprint_node()
    );
    assert!(Definition::from_registrations(&plans, source_unit()).is_none());
}

#[test]
fn local_and_dependency_roles_keep_the_same_frozen_id_sequence_wire() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let definition = Definition::from_registrations(&plans, fixture.unit).unwrap();
    let mut expected = vec![0x81];
    expected.extend(encode(&fixture.unit).unwrap());
    for consumer in [ConeIdentity::SINGLE_FILE, ConeIdentity::CORE] {
        let catalog = Catalog::new(consumer, &[definition], &mut meter()).unwrap();
        let resolved = catalog
            .resolve(source_unit(), &[decoded(fixture.unit)], &mut meter())
            .unwrap();
        assert_eq!(resolved.local_unit(), source_unit());
        assert_eq!(encode(&resolved).unwrap(), expected);
        assert_eq!(resolved.references()[0].definition(), &definition);
        match resolved.references()[0].kind() {
            Kind::LocalUnit(unit) => {
                assert_eq!(consumer, ConeIdentity::SINGLE_FILE);
                assert_eq!(unit, &definition);
            }
            Kind::DependencyExternalUnit { provider, unit_ref } => {
                assert_eq!(consumer, ConeIdentity::CORE);
                assert_eq!(provider, ConeIdentity::SINGLE_FILE);
                assert_eq!(unit_ref, &definition);
            }
        }
    }
}

#[test]
fn missing_duplicate_self_and_noncanonical_dependencies_are_rejected() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let definition = Definition::from_registrations(&plans, fixture.unit).unwrap();
    assert!(
        matches!(Catalog::new(ConeIdentity::CORE, &[definition, definition], &mut meter()),
        Err(Error::DuplicateDefinition(unit)) if unit == fixture.unit)
    );
    let catalog = Catalog::new(ConeIdentity::CORE, &[definition], &mut meter()).unwrap();
    assert!(
        matches!(catalog.resolve(fixture.unit, &[decoded(fixture.unit)], &mut meter()),
        Err(Error::SelfDependency(unit)) if unit == fixture.unit)
    );
    let unknown = PersistentInitializationUnitId::from_key(
        &InitializationUnitKey::TopLevelProperty(property("unknown")),
    )
    .unwrap();
    assert!(
        matches!(catalog.resolve(source_unit(), &[decoded(unknown)], &mut meter()),
        Err(Error::UnknownUnit(unit)) if unit == decoded(unknown))
    );
    assert!(matches!(
        catalog.resolve(
            source_unit(),
            &[decoded(fixture.unit), decoded(fixture.unit)],
            &mut meter()
        ),
        Err(Error::NonCanonicalOrder { index: 1 })
    ));
    let mut budget = BudgetMeter::new(DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        catalog.resolve(source_unit(), &[decoded(fixture.unit)], &mut budget),
        Err(Error::Resource(_))
    ));
}
