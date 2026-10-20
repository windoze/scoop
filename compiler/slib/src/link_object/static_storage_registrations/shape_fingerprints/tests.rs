use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

#[test]
fn computes_static_layout_and_scan_leaves_from_the_closed_plan() {
    let fixture = Fixture::new(Corruption::None);
    let definitions = storage_definitions(&fixture);

    let fingerprints = compute_strong_static_storage_shape_fingerprints_v1(definitions).unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    assert_eq!(actual.storage(), plan.semantic().storage());
    assert_eq!(actual.layout(), plan.semantic().layout());
    assert_eq!(actual.layout_node(), plan.layout_fingerprint_node());
    assert_eq!(actual.scan(), plan.semantic().scan());
    assert_eq!(actual.scan_node(), plan.scan_fingerprint_node());
    assert_eq!(
        actual.layout_fingerprint().to_string(),
        "0dbc2daab5ec274c4ff152bb119cdfaa7bab9ec28ed7e4767d279f810c8f681b"
    );
    assert_eq!(
        actual.scan_fingerprint().to_string(),
        "088178827b421127a3fef5922950fa619d8715cddb28dbb2963995a09c4dee75"
    );
}

#[test]
fn layout_leaf_commits_size_and_alignment() {
    let fixture = Fixture::new(Corruption::None);
    let layout = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .layout();
    let baseline = static_layout_fingerprint(layout, 8, 8).unwrap();

    assert_ne!(baseline, static_layout_fingerprint(layout, 16, 8).unwrap());
    assert_ne!(baseline, static_layout_fingerprint(layout, 8, 16).unwrap());
}

#[test]
fn scan_leaf_commits_the_canonical_reference_offsets() {
    let empty = scan_fingerprint(&RefScan::None).unwrap();
    let first = scan_fingerprint(&RefScan::References(vec![0])).unwrap();
    let second = scan_fingerprint(&RefScan::References(vec![8])).unwrap();

    assert_ne!(empty, first);
    assert_ne!(first, second);
}

fn storage_definitions(fixture: &Fixture) -> StaticStorageRegistrationDataV1 {
    fixture.static_storage_registration_plan.clone()
}
