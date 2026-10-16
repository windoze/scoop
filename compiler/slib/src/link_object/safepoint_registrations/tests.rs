use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

#[test]
fn binds_the_planned_digest_slots() {
    let fixture = Fixture::new(Corruption::None);
    let data = bind_safepoint_registrations_v1(
        fixture.verified_stackmaps(),
        fixture.verified_patch_sites(),
        fixture.registration_plan.clone(),
    )
    .unwrap();
    assert_eq!(data.registrations().len(), 2);
    for (record, planned) in data.registrations().iter().zip(data.plan().registrations()) {
        assert_eq!(record.site(), planned.site());
        assert_eq!(record.member(), fixture.member);
        assert_eq!(
            record.normalized_stackmap_patch().intent(),
            planned.normalized_stackmap_patch()
        );
    }
}
