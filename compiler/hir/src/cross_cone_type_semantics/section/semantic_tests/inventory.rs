use super::*;

#[test]
fn source_only_concrete_root_preserves_lexical_metadata_without_concrete_support() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    let root = fixture.add("SourceOnly", false);
    let candidate = fixture.section(vec![]);
    let public = public();
    let checked = check(&fixture, &candidate, &public, &[], &Uses::default()).unwrap();
    assert_eq!(checked.source_roots(), &[root.source]);
    assert!(checked.graph().get(root.exact).is_none());
    assert!(checked.facts().records().is_empty());
}

#[test]
fn complete_nominal_support_requires_each_of_the_three_local_tables() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    fixture.add("Complete", true);
    let public = public();
    let candidate = fixture.section(vec![]);
    let checked = check(&fixture, &candidate, &public, &[], &Uses::default()).unwrap();
    assert_eq!(checked.facts().records().len(), 1);
    assert_eq!(checked.representations().table().records().len(), 1);
    for field in 1..=3 {
        let mut bad = candidate.clone();
        match field {
            1 => bad.exact_facts = Default::default(),
            2 => bad.representation_support = Default::default(),
            3 => bad.inheritance = Default::default(),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                check(&fixture, &bad, &public, &[], &Uses::default()),
                Err(TypeSectionSemanticValidationError::Exports(_))
            ),
            "field {field}"
        );
    }
}

#[test]
fn candidate_cannot_delete_independently_required_concrete_inventory() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    let root = fixture.add("Required", false);
    let candidate = fixture.section(vec![]);
    let SourceNominalId::Concrete(owner) = root.source else {
        unreachable!()
    };
    fixture.representations = CanonicalPersistentIdsV1::try_new(vec![owner]).unwrap();
    assert!(check(&fixture, &candidate, &public(), &[], &Uses::default()).is_err());
    fixture.representations = CanonicalPersistentIdsV1::empty();
    fixture
        .edges
        .push(fixture.source.graph.records[&root.exact].clone());
    assert!(check(&fixture, &candidate, &public(), &[], &Uses::default()).is_err());
}

#[test]
fn explicit_source_roots_and_exact_fact_inventory_are_not_recovered_from_candidate() {
    let mut fixture = Fixture::new(ConeIdentity::CORE);
    fixture.add("Root", true);
    let candidate = fixture.section(vec![]);
    fixture.roots.clear();
    assert!(check(&fixture, &candidate, &public(), &[], &Uses::default()).is_err());
}
