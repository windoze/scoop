use super::*;
use crate::cross_cone_type_bridge::tests::support::Fixture;

#[test]
fn borrowed_type_index_keeps_canonical_tables_separate() {
    let fixture = Fixture::new();
    let source = fixture.empty_export();
    let helper = fixture.boxed_export();
    let local = CanonicalParamFreeMirTypeExportsV1::try_new(vec![helper.clone()]).unwrap();
    let dependency = CanonicalParamFreeMirTypeExportsV1::try_new(vec![source.clone()]).unwrap();
    let lookup = MirTypeBridgeTypeIndexV1::try_new(&[&local, &dependency]).unwrap();
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
fn borrowed_index_rejects_duplicate_strong_types_and_generated_helpers() {
    let fixture = Fixture::new();
    for record in [fixture.empty_export(), fixture.boxed_export()] {
        let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![record]).unwrap();
        assert!(matches!(
            MirTypeBridgeTypeIndexV1::try_new(&[&table, &table]),
            Err(MirTypeBridgeLookupError::DuplicateType { .. })
        ));
    }
}
