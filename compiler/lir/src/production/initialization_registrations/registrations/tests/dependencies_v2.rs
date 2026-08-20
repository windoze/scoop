use super::*;
use crate::{
    InitializationDependencyResolutionError as Error,
    StrongInitializationDefinitionCatalogV2 as Catalog,
    StrongInitializationDependencyKindV2 as Kind,
    StrongInitializationUnitDefinitionRefV2 as Definition,
};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{decode_canonical, encode};

fn decoded(
    unit: PersistentInitializationUnitId,
) -> DecodedPersistentId<PersistentInitializationUnitId> {
    decode_canonical(&encode(&unit).unwrap()).unwrap()
}
fn source_unit() -> PersistentInitializationUnitId {
    PersistentInitializationUnitId::from_key(&InitializationUnitKey::TopLevelProperty(property(
        "source",
    )))
    .unwrap()
}

#[test]
fn dependency_definitions_retain_provider_and_cell_and_registration_relations() {
    let fixture = Fixture::new(Options::default());
    let plans = fixture.build().unwrap();
    let definition = Definition::from_registrations(&plans, fixture.unit).unwrap();
    let plan = &plans.registrations()[0];
    assert_eq!(definition.provider(), plans.producer());
    assert_eq!(definition.unit(), fixture.unit);
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
        let catalog = Catalog::new(consumer, &[definition]).unwrap();
        let resolved = catalog
            .resolve(source_unit(), &[decoded(fixture.unit)])
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
        matches!(Catalog::new(ConeIdentity::CORE, &[definition, definition]),
        Err(Error::DuplicateDefinition(unit)) if unit == fixture.unit)
    );
    let catalog = Catalog::new(ConeIdentity::CORE, &[definition]).unwrap();
    assert!(
        matches!(catalog.resolve(fixture.unit, &[decoded(fixture.unit)]),
        Err(Error::SelfDependency(unit)) if unit == fixture.unit)
    );
    let unknown = PersistentInitializationUnitId::from_key(
        &InitializationUnitKey::TopLevelProperty(property("unknown")),
    )
    .unwrap();
    assert!(
        matches!(catalog.resolve(source_unit(), &[decoded(unknown)]),
        Err(Error::UnknownUnit(unit)) if unit == decoded(unknown))
    );
    assert!(matches!(
        catalog.resolve(
            source_unit(),
            &[decoded(fixture.unit), decoded(fixture.unit)]
        ),
        Err(Error::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn unit_definitions_resolve_before_dependency_semantics_for_both_schedules() {
    for lazy in [false, true] {
        for provider in [ConeIdentity::SINGLE_FILE, ConeIdentity::CORE] {
            let fixture = Fixture::with_source(
                Options {
                    lazy,
                    ..Options::default()
                },
                provider,
                "definition",
            );
            let before =
                Definition::from_foundation(fixture.unit, &fixture.foundation, &fixture.identities)
                    .unwrap();
            let complete = fixture.build().unwrap();
            assert_eq!(
                before,
                Definition::from_registrations(&complete, fixture.unit).unwrap()
            );
            assert_eq!(before.provider(), provider);
        }
    }
}

#[test]
fn definition_resolution_rejects_missing_unit_registration_and_physical_parts() {
    use crate::InitializationDefinitionResolutionErrorV2 as DefinitionError;
    let fixture = Fixture::new(Options::default());
    assert!(matches!(
        Definition::from_foundation(source_unit(), &fixture.foundation, &fixture.identities,),
        Err(DefinitionError::MissingRegistrationIdentity(_))
    ));
    let missing_registration = Fixture::new(Options {
        omit_unit_registration: true,
        ..Options::default()
    });
    assert!(matches!(
        Definition::from_foundation(
            missing_registration.unit,
            &missing_registration.foundation,
            &missing_registration.identities,
        ),
        Err(DefinitionError::MissingRegistrationIdentity(_))
    ));
    for (options, expected) in [
        (
            Options {
                omit_cell_symbol: true,
                ..Options::default()
            },
            "symbol",
        ),
        (
            Options {
                omit_registration_primary: true,
                ..Options::default()
            },
            "primary",
        ),
        (
            Options {
                omit_diagnostic_atom: true,
                ..Options::default()
            },
            "associated",
        ),
    ] {
        let fixture = Fixture::new(options);
        let error =
            Definition::from_foundation(fixture.unit, &fixture.foundation, &fixture.identities)
                .unwrap_err();
        assert!(matches!(
            (error, expected),
            (DefinitionError::MissingSymbol(_), "symbol")
                | (DefinitionError::PrimaryAtoms(_), "primary")
                | (DefinitionError::AssociatedAtoms(_), "associated")
        ));
    }
}
