use super::*;

type Edge = (
    PersistentInitializationUnitId,
    ConeIdentity,
    PersistentInitializationUnitId,
);

fn uses(fixture: &Fixture) -> CanonicalMirExternalInitializationUsesV1 {
    CanonicalMirExternalInitializationUsesV1::try_new(vec![
        fixture
            .use_for(
                1,
                MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
            )
            .unwrap(),
        fixture
            .use_for(
                1,
                MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[1].id()),
            )
            .unwrap(),
        fixture
            .use_for(
                2,
                MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[0].id()),
            )
            .unwrap(),
    ])
    .unwrap()
}

fn edges(fixture: &Fixture) -> [Edge; 2] {
    [1, 2].map(|index| {
        (
            fixture.units[0].id(),
            ConeIdentity::CORE,
            fixture.units[index].id(),
        )
    })
}

#[test]
fn registration_edges_merge_distinct_causes_for_the_same_unit_dependency() {
    let fixture = Fixture::new();
    let uses = uses(&fixture);
    assert_eq!(uses.records().len(), 3);
    uses.validate_registration_edges(edges(&fixture)).unwrap();
    uses.validate_registration_edges(edges(&fixture).into_iter().rev())
        .unwrap();
    CanonicalMirExternalInitializationUsesV1::try_new(vec![])
        .unwrap()
        .validate_registration_edges([])
        .unwrap();
}

#[test]
fn registration_edges_reject_missing_extra_and_changed_typed_endpoints() {
    let fixture = Fixture::new();
    let uses = uses(&fixture);
    let [first, second] = edges(&fixture);
    for actual in [
        vec![],
        vec![first],
        vec![first, second, (first.0, first.1, fixture.units[4].id())],
        vec![(first.0, ConeIdentity::SINGLE_FILE, first.2), second],
        vec![(fixture.units[1].id(), first.1, first.2), second],
    ] {
        assert!(matches!(
            uses.validate_registration_edges(actual),
            Err(MirObjectBridgeError::InitializationDependencyInventory)
        ));
    }
}

#[test]
fn registration_edges_reject_duplicate_edges_instead_of_deduplicating_them() {
    let fixture = Fixture::new();
    let [first, second] = edges(&fixture);
    assert!(matches!(
        uses(&fixture).validate_registration_edges([first, second, first]),
        Err(MirObjectBridgeError::DuplicateInitializationDependency)
    ));
}
