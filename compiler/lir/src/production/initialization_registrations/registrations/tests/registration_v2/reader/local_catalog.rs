use super::*;

#[test]
fn registration_catalog_derives_local_definitions_and_preserves_external_ones() {
    let local = Fixture::new(Options::default());
    let external = Fixture::with_source(Options::default(), ConeIdentity::CORE, "external");
    let source = Fixture::with_source(Options::default(), ConeIdentity::SINGLE_FILE, "source");
    let catalog = Catalog::new(
        ConeIdentity::SINGLE_FILE,
        &[definition(&external)],
        &mut meter(),
    )
    .unwrap();
    let complete = catalog
        .with_local_foundation(
            &local.foundation,
            &local.identities,
            &local.digests,
            &mut meter(),
        )
        .unwrap();
    for (fixture, is_local) in [(&local, true), (&external, false)] {
        let id =
            decode_canonical(&encode(&fixture.unit).unwrap(), DecodeLimits::default()).unwrap();
        let resolved = complete.resolve(source.unit, &[id], &mut meter()).unwrap();
        let reference = &resolved.references()[0];
        assert_eq!(reference.definition(), &definition(fixture));
        assert_eq!(matches!(reference.kind(), Kind::LocalUnit(_)), is_local);
    }
}

#[test]
fn registration_catalog_rejects_injected_local_definitions_and_wrong_consumers() {
    let fixture = Fixture::new(Options::default());
    let catalog = Catalog::new(
        ConeIdentity::SINGLE_FILE,
        &[definition(&fixture)],
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        catalog.with_local_foundation(&fixture.foundation, &fixture.identities, &fixture.digests, &mut meter()),
        Err(Error::InitializationDependency(crate::InitializationDependencyResolutionError::CurrentConeDefinition(unit)))
            if unit == fixture.unit
    ));
    let wrong = Catalog::new(ConeIdentity::CORE, &[], &mut meter()).unwrap();
    assert!(matches!(
        wrong.with_local_foundation(
            &fixture.foundation,
            &fixture.identities,
            &fixture.digests,
            &mut meter()
        ),
        Err(Error::ProducerMismatch)
    ));
}

#[test]
fn registration_catalog_rejects_incomplete_local_definitions_and_exhausted_budgets() {
    let catalog = Catalog::new(ConeIdentity::SINGLE_FILE, &[], &mut meter()).unwrap();
    let incomplete = Fixture::new(Options {
        omit_descriptor_primary: true,
        ..Options::default()
    });
    assert!(matches!(
        catalog.with_local_foundation(
            &incomplete.foundation,
            &incomplete.identities,
            &incomplete.digests,
            &mut meter()
        ),
        Err(Error::InitializationDefinition(_))
    ));
    let fixture = Fixture::new(Options::default());
    let replay = |meter: &mut BudgetMeter| {
        catalog.with_local_foundation(
            &fixture.foundation,
            &fixture.identities,
            &fixture.digests,
            meter,
        )
    };
    for limits in [
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(replay(&mut BudgetMeter::new(limits)).is_err());
    }
    let mut measured = meter();
    replay(&mut measured).unwrap();
    let mut cumulative = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&mut cumulative).unwrap();
    assert!(replay(&mut cumulative).is_err());
}
