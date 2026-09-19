use super::*;
use crate::cross_cone_type_bridge::tests::support::Fixture;
use scoop_wire::DecodeLimits;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn borrowed_type_index_keeps_canonical_tables_separate() {
    let fixture = Fixture::new();
    let source = fixture.empty_export();
    let helper = fixture.boxed_export();
    let local = CanonicalParamFreeMirTypeExportsV1::try_new(vec![helper.clone()]).unwrap();
    let dependency = CanonicalParamFreeMirTypeExportsV1::try_new(vec![source.clone()]).unwrap();
    let lookup = MirTypeBridgeTypeIndexV1::try_new(&[&local, &dependency], &mut meter()).unwrap();
    assert_eq!(lookup.record_count(), 2);
    assert!(std::ptr::eq(
        lookup.get(source.exact()).unwrap(),
        &dependency.records()[0]
    ));
    assert!(std::ptr::eq(
        lookup.get(helper.exact()).unwrap(),
        &local.records()[0]
    ));
    assert!(local.get(source.exact()).is_none());
    assert!(dependency.get(helper.exact()).is_none());
    assert_eq!(local.records().len(), 1);
}

#[test]
fn borrowed_index_rejects_duplicate_authority_even_with_identical_records() {
    let fixture = Fixture::new();
    let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![fixture.empty_export()]).unwrap();
    assert!(matches!(
        MirTypeBridgeTypeIndexV1::try_new(&[&table, &table], &mut meter()),
        Err(MirTypeBridgeLookupError::DuplicateType { .. })
    ));
}

#[test]
fn borrowed_index_accounts_for_allocation_and_sorting_work() {
    let fixture = Fixture::new();
    let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        fixture.empty_export(),
        fixture.boxed_export(),
    ])
    .unwrap();
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
        assert!(matches!(
            MirTypeBridgeTypeIndexV1::try_new(&[&table], &mut BudgetMeter::new(limits)),
            Err(MirTypeBridgeLookupError::Resource(_))
        ));
    }
    let mut measured = meter();
    MirTypeBridgeTypeIndexV1::try_new(&[&table], &mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    MirTypeBridgeTypeIndexV1::try_new(
        &[&table],
        &mut BudgetMeter::new(DecodeLimits {
            validation_work_units: required,
            ..DecodeLimits::default()
        }),
    )
    .unwrap();
}
