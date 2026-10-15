use super::*;

mod fixture;
use fixture::Fixture;

#[test]
fn owned_lir_dependency_graph_resolves_explicit_symbol_roots() {
    let fixture = Fixture::new();
    let expected = fixture.relations();
    let resolved = fixture.resolve(&expected).unwrap();
    assert_eq!(resolved.exports(), &fixture.local);
    assert_eq!(resolved.selected_relations(), expected);
    let leaf = section(fixture.leaf.clone(), &[], &[]).unwrap();
    let middle = section(fixture.middle.clone(), &[leaf.exports()], &[]).unwrap();
    let complete = section(
        fixture.local.clone(),
        &[middle.exports(), leaf.exports()],
        &fixture.relations(),
    )
    .unwrap();
    assert_eq!(
        complete.selected().semantic_relations().collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn owned_lir_dependency_records_require_canonical_external_references() {
    let fixture = Fixture::new();
    let expected = fixture.relations();
    let mut reversed = expected.clone();
    reversed.reverse();
    let duplicate = vec![expected[0], expected[0]];
    for candidate in [reversed, duplicate] {
        assert!(matches!(
            fixture.resolve(&candidate),
            Err(LayoutAbiSectionError::NonCanonicalSelected { index: 1 })
        ));
    }
    let local = LayoutAbiDependencyV1::new(fixture.local.provider(), fixture.middle_use().target());
    assert!(matches!(
        fixture.resolve(&[local]),
        Err(LayoutAbiSectionError::SelectedCurrentProvider)
    ));
}
