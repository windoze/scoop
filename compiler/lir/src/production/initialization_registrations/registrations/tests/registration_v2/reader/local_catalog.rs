use super::*;

#[test]
fn registration_catalog_derives_local_definitions_and_preserves_external_ones() {
    let local = Fixture::new(Options::default());
    let external = Fixture::with_source(Options::default(), ConeIdentity::CORE, "external");
    let source = Fixture::with_source(Options::default(), ConeIdentity::SINGLE_FILE, "source");
    let catalog = Catalog::new(ConeIdentity::SINGLE_FILE, &[definition(&external)]).unwrap();
    let complete = catalog
        .with_local_foundation(&local.foundation, &local.identities)
        .unwrap();
    for (fixture, is_local) in [(&local, true), (&external, false)] {
        let id = decode_canonical(&encode(&fixture.unit).unwrap()).unwrap();
        let resolved = complete.resolve(source.unit, &[id]).unwrap();
        let reference = &resolved.references()[0];
        assert_eq!(reference.definition(), &definition(fixture));
        assert_eq!(matches!(reference.kind(), Kind::LocalUnit(_)), is_local);
    }
}

#[test]
fn registration_catalog_rejects_injected_local_definitions_and_wrong_consumers() {
    let fixture = Fixture::new(Options::default());
    let catalog = Catalog::new(ConeIdentity::SINGLE_FILE, &[definition(&fixture)]).unwrap();
    assert!(matches!(
        catalog.with_local_foundation(&fixture.foundation, &fixture.identities),
        Err(Error::InitializationDependency(crate::InitializationDependencyResolutionError::CurrentConeDefinition(unit)))
            if unit == fixture.unit
    ));
    let wrong = Catalog::new(ConeIdentity::CORE, &[]).unwrap();
    assert!(matches!(
        wrong.with_local_foundation(&fixture.foundation, &fixture.identities),
        Err(Error::ProducerMismatch)
    ));
}
