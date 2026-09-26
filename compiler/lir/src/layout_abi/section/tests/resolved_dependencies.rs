use super::*;
use std::convert::Infallible;

mod fixture;
use fixture::Fixture;

#[test]
fn owned_lir_dependency_graph_replays_explicit_roots_and_transitive_fields() {
    let fixture = Fixture::new();
    let expected = fixture.relations();
    let resolved = fixture.resolve(&expected).unwrap();
    assert_eq!(resolved.exports(), &fixture.local);
    assert_eq!(resolved.selected_relations(), expected);
    resolved
        .replay_dependency_closure::<Infallible>(&fixture.dependencies(), &[fixture.middle_use()])
        .unwrap();
    assert!(matches!(
        resolved.replay_dependency_closure::<Infallible>(&fixture.dependencies(), &[]),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));

    let leaf = section(fixture.leaf.clone(), &[], &Source::default()).unwrap();
    let middle = section(
        fixture.middle.clone(),
        &[leaf.exports()],
        &Source::default(),
    )
    .unwrap();
    let complete = section(
        fixture.local.clone(),
        &[middle.exports(), leaf.exports()],
        &Source(vec![fixture.middle_use()]),
    )
    .unwrap();
    assert_eq!(
        complete.selected().semantic_relations().collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn owned_lir_dependency_graph_also_closes_every_local_export() {
    let fixture = Fixture::new();
    let resolved = fixture
        .resolve_with(&fixture.middle, &[fixture.leaf_use()])
        .unwrap();
    resolved
        .replay_dependency_closure::<Infallible>(&[&fixture.leaf], &[])
        .unwrap();
    let missing = fixture.resolve_with(&fixture.middle, &[]).unwrap();
    assert!(matches!(
        missing.replay_dependency_closure::<Infallible>(&[&fixture.leaf], &[]),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
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
            resolved.replay_dependency_closure::<Infallible>(
                &fixture.dependencies(),
                &[fixture.middle_use()]
            ),
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
        resolved.replay_dependency_closure::<Infallible>(
            &fixture.dependencies(),
            &[fixture.middle_use()]
        ),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
}

#[test]
fn owned_lir_dependency_graph_rejects_missing_duplicate_and_wrong_providers() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve(&fixture.relations()).unwrap();
    let replay = |dependencies: &[_], roots: &[_]| {
        resolved.replay_dependency_closure::<Infallible>(dependencies, roots)
    };
    assert!(matches!(
        replay(&[&fixture.middle], &[fixture.middle_use()]),
        Err(LayoutAbiSectionError::Semantic(
            LayoutAbiSemanticClosureError::MissingTarget(_)
        ))
    ));
    assert!(matches!(
        replay(&[&fixture.leaf, &fixture.leaf], &[fixture.middle_use()]),
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
