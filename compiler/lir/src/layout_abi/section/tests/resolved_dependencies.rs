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
    resolved
        .replay_dependency_closure(&fixture.dependencies(), &fixture.relations())
        .unwrap();
    assert!(matches!(
        resolved.replay_dependency_closure(&fixture.dependencies(), &[]),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));

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
fn local_field_storage_requires_no_additional_symbol_selection() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve_with(&fixture.middle, &[]).unwrap();
    resolved.replay_dependency_closure(&[], &[]).unwrap();
    assert!(resolved.selected_relations().is_empty());
}

#[test]
fn owned_lir_dependency_graph_rejects_missing_extra_and_noncanonical_selected() {
    let fixture = Fixture::new();
    let expected = fixture.relations();
    for index in 0..expected.len() {
        let mut missing = expected.clone();
        missing.remove(index);
        let resolved = fixture.resolve(&missing).unwrap();
        assert!(matches!(
            resolved.replay_dependency_closure(&fixture.dependencies(), &fixture.relations()),
            Err(LayoutAbiSectionError::SelectedClosure)
        ));
    }
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
    let wrong = LayoutAbiDependencyV1::new(fixture.leaf.provider(), fixture.middle_use().target());
    let resolved = fixture.resolve(&[wrong]).unwrap();
    assert!(matches!(
        resolved.replay_dependency_closure(&fixture.dependencies(), &fixture.relations()),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
}

#[test]
fn owned_lir_dependency_graph_rejects_missing_duplicate_and_wrong_providers() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve(&fixture.relations()).unwrap();
    let replay =
        |dependencies: &[_], roots: &[_]| resolved.replay_dependency_closure(dependencies, roots);
    assert!(matches!(
        replay(&[&fixture.middle], &fixture.relations()),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::MissingTarget(_)
        ))
    ));
    assert!(matches!(
        replay(&[&fixture.leaf, &fixture.leaf], &fixture.relations()),
        Err(LayoutAbiSectionError::DuplicateProvider(provider)) if provider == fixture.leaf.provider()
    ));
    assert!(matches!(
        replay(&[&fixture.local], &[]),
        Err(LayoutAbiSectionError::DuplicateProvider(provider)) if provider == fixture.local.provider()
    ));
    let wrong = LayoutAbiDependencyV1::new(fixture.leaf.provider(), fixture.middle_use().target());
    assert!(matches!(
        replay(&fixture.dependencies(), &[wrong]),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::Provider { .. }
        ))
    ));
    let current =
        LayoutAbiDependencyV1::new(fixture.local.provider(), fixture.middle_use().target());
    assert!(matches!(
        replay(&fixture.dependencies(), &[current]),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::CurrentProvider(_)
        ))
    ));
    assert!(matches!(
        replay(
            &fixture.dependencies(),
            &[fixture.middle_use(), fixture.middle_use()]
        ),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::NonCanonicalRoots
        ))
    ));
}
