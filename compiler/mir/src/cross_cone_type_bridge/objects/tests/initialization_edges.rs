use super::*;

type Edge = (
    PersistentInitializationUnitId,
    ConeIdentity,
    PersistentInitializationUnitId,
);

fn uses(fixture: &Fixture) -> CanonicalMirExternalInitializationUsesV1 {
    CanonicalMirExternalInitializationUsesV1::try_new(
        vec![
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
        ],
        &mut meter(),
    )
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
    uses.validate_registration_edges(edges(&fixture), &mut meter())
        .unwrap();
    uses.validate_registration_edges(edges(&fixture).into_iter().rev(), &mut meter())
        .unwrap();
    CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter())
        .unwrap()
        .validate_registration_edges([], &mut meter())
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
            uses.validate_registration_edges(actual, &mut meter()),
            Err(MirObjectBridgeError::InitializationDependencyInventory)
        ));
    }
}

#[test]
fn registration_edges_reject_duplicate_edges_instead_of_deduplicating_them() {
    let fixture = Fixture::new();
    let [first, second] = edges(&fixture);
    assert!(matches!(
        uses(&fixture).validate_registration_edges([first, second, first], &mut meter()),
        Err(MirObjectBridgeError::DuplicateInitializationDependency)
    ));
}

#[test]
fn registration_edges_charge_original_work_allocation_and_table_budgets() {
    let fixture = Fixture::new();
    let uses = uses(&fixture);
    let mut measured = meter();
    uses.validate_registration_edges(edges(&fixture), &mut measured)
        .unwrap();
    let required = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    uses.validate_registration_edges(edges(&fixture), &mut exact)
        .unwrap();
    assert!(matches!(
        uses.validate_registration_edges(edges(&fixture), &mut exact),
        Err(MirObjectBridgeError::Resource(_))
    ));
    for limits in [
        DecodeLimits {
            validation_work_units: required - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            uses.validate_registration_edges(edges(&fixture), &mut BudgetMeter::new(limits)),
            Err(MirObjectBridgeError::Resource(_))
        ));
    }
}
