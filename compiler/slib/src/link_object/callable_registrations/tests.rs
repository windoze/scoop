use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

#[test]
fn binds_the_planned_digest_slots() {
    let fixture = Fixture::new(Corruption::None);
    let data = bind_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
    )
    .unwrap();
    assert_eq!(data.registrations().len(), 1);
    for (record, planned) in data.registrations().iter().zip(data.plan().registrations()) {
        assert_eq!(record.body(), planned.body());
        assert_eq!(record.member(), fixture.member);
        assert_eq!(
            record.body_definition_patch().intent(),
            planned.body_definition_patch()
        );
    }
}
