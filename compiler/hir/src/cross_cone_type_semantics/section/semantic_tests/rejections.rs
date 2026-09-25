use super::*;

#[test]
fn independently_valid_fact_and_representation_tables_must_join_same_source_shape() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    let root = fixture.add("NotScalar", true);
    let mut wire = fixture.section(vec![]);
    fixture
        .shapes
        .insert(root.exact, ExactTypeFactShapeV1::Scalar);
    wire.exact_facts = CanonicalExactTypeFactsV1::try_new(vec![
        ExactTypeFactsV1::try_new(
            root.exact,
            ExactTypeKindV1::Value {
                zst: ZstStatus::NonZero,
            },
            ExactTypeGcV1::GcFree,
        )
        .unwrap(),
    ])
    .unwrap();
    assert!(
        matches!(check(&fixture, &wire, &public(), &[], &Uses::default()), Err(TypeSectionSemanticValidationError::Exports(error))
        if matches!(*error, TypeSectionExportValidationError::FactShape(exact) if exact == root.exact))
    );
}

#[test]
fn foreign_records_cannot_be_copied_into_local_inventory_or_reassigned_by_exact_shape() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let root = provider.add("Foreign", true);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let mut wire = consumer.section(vec![]);
    wire.exact_facts = provider_wire.exact_facts.clone();
    assert!(check(&consumer, &wire, &public, &[&terminal], &Uses::default()).is_err());
    let wire = consumer.section(vec![]);
    consumer.foreign.push(TypeSectionDependencyFactV1 {
        provider: ConeIdentity::SINGLE_FILE,
        exact: root.exact,
    });
    assert!(
        matches!(check(&consumer, &wire, &public, &[&terminal], &Uses::default()), Err(TypeSectionSemanticValidationError::Exports(error))
        if matches!(*error, TypeSectionExportValidationError::DependencyFact(exact) if exact == root.exact))
    );
    consumer.foreign[0].provider = ConeIdentity::CORE;
    assert!(check(&consumer, &wire, &public, &[&terminal], &Uses::default()).is_ok());
}

#[test]
fn terminal_provider_set_and_public_token_must_match_the_independent_owner() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    fixture.add("Root", true);
    let wire = fixture.section(vec![]);
    let public = public();
    let terminal = check(&fixture, &wire, &public, &[], &Uses::default()).unwrap();
    assert!(
        matches!(check(&fixture, &wire, &public, &[&terminal], &Uses::default()), Err(TypeSectionSemanticValidationError::Exports(error))
        if matches!(*error, TypeSectionExportValidationError::DependencyOrder))
    );
    let error = wire
        .validate_semantics(
            public_proof(&public, ConeIdentity::SINGLE_FILE),
            &[],
            &fixture,
            &mut fixture.source.clone(),
            &mut DefaultAuthority::new(&fixture),
            &Uses::default(),
            &path(),
        )
        .unwrap_err();
    assert!(
        matches!(error, TypeSectionSemanticValidationError::Exports(error)
        if matches!(*error, TypeSectionExportValidationError::Provider))
    );
}

#[test]
fn structural_exact_target_cannot_obtain_nominal_materialization_capability() {
    let mut provider = Fixture::new(ConeIdentity::CORE);
    let root = provider.add("Element", true);
    let provider_wire = provider.section(vec![]);
    let public = public();
    let terminal = check(&provider, &provider_wire, &public, &[], &Uses::default()).unwrap();
    let mut consumer = Fixture::new(ConeIdentity::SINGLE_FILE);
    consumer.import(&provider);
    let key = ExactTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(root.exact, []));
    let exact = PersistentExactTypeId::from_key(&key).unwrap();
    consumer.source.graph.exacts.insert(exact, key);
    let request = SelectedExternalTypeUseV1::new(
        ConeIdentity::CORE,
        SelectedTypeUseV1::Representation { exact },
    );
    let wire = consumer.section(vec![request]);
    assert!(
        matches!(check(&consumer, &wire, &public, &[&terminal], &Uses::new(&[request])), Err(TypeSectionSemanticValidationError::Selected(error))
        if matches!(*error, TypeSelectionValidationError::RequiresOdr(value) if value == exact))
    );
}
