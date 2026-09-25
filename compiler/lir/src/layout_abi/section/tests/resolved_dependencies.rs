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
        .replay_dependency_closure::<Infallible>(
            &fixture.dependencies(),
            &[fixture.middle_use()],
            &mut meter(),
        )
        .unwrap();
    assert!(matches!(
        resolved.replay_dependency_closure::<Infallible>(
            &fixture.dependencies(),
            &[],
            &mut meter()
        ),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));

    let leaf = section(fixture.leaf.clone(), &[], &Source::default()).unwrap();
    let middle = section(fixture.middle.clone(), &[&leaf], &Source::default()).unwrap();
    let complete = section(
        fixture.local.clone(),
        &[&middle],
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
        .replay_dependency_closure::<Infallible>(&[&fixture.leaf], &[], &mut meter())
        .unwrap();
    let missing = fixture.resolve_with(&fixture.middle, &[]).unwrap();
    assert!(matches!(
        missing.replay_dependency_closure::<Infallible>(&[&fixture.leaf], &[], &mut meter()),
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
                &[fixture.middle_use()],
                &mut meter()
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
            &[fixture.middle_use()],
            &mut meter()
        ),
        Err(LayoutAbiSectionError::SelectedClosure)
    ));
}

#[test]
fn owned_lir_dependency_graph_rejects_missing_duplicate_and_wrong_providers() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve(&fixture.relations()).unwrap();
    let replay = |dependencies: &[_], roots: &[_]| {
        resolved.replay_dependency_closure::<Infallible>(dependencies, roots, &mut meter())
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

#[test]
fn owned_lir_dependency_graph_charges_inclusive_and_cumulative_budgets() {
    let fixture = Fixture::new();
    let resolved = fixture.resolve(&fixture.relations()).unwrap();
    let replay = |meter: &mut BudgetMeter| {
        resolved.replay_dependency_closure::<Infallible>(
            &fixture.dependencies(),
            &[fixture.middle_use()],
            meter,
        )
    };
    let mut measured = meter();
    replay(&mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    let mut inclusive = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    replay(&mut inclusive).unwrap();
    assert!(replay(&mut inclusive).is_err());
    for limits in [
        DecodeLimits {
            validation_work_units: required - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(replay(&mut BudgetMeter::new(limits)).is_err());
    }
}
