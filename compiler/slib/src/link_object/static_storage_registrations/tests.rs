use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

#[test]
fn binds_storage_scan_and_layout_slots() {
    let fixture = Fixture::new(Corruption::None);
    let data = bind_static_storage_registrations_v1(
        &fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
    )
    .unwrap();
    assert_eq!(data.registrations().len(), 1);
    let actual = &data.registrations()[0];
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    assert_eq!(actual.storage(), plan.semantic().storage());
    assert_eq!(actual.member(), fixture.member);
    assert_eq!(
        actual.scan_fingerprint_patch().intent(),
        plan.scan_fingerprint_patch()
    );
    assert_eq!(
        actual.layout_fingerprint_patch().intent(),
        plan.layout_fingerprint_patch()
    );
    assert_eq!(
        actual.layout_fingerprint_patch().checked_offset(),
        actual.scan_fingerprint_patch().checked_offset() + 32
    );
}
