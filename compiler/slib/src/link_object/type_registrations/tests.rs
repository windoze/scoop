use super::*;
use crate::link_object::ScoopLirObjectCandidateV1;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
mod v2;

#[test]
fn binds_descriptor_and_layout_digest_slots() {
    let fixture = Fixture::new(Corruption::None);
    let data = bind_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
    )
    .unwrap();
    assert_eq!(data.registrations().len(), 1);
    let actual = &data.registrations()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(actual.exact_type(), plan.exact_type());
    assert_eq!(actual.member(), fixture.member);
    assert_eq!(
        actual.descriptor_definition_patch().intent(),
        plan.descriptor_definition_patch()
    );
    assert_eq!(
        actual.layout_fingerprint_patch().intent(),
        plan.layout_fingerprint_patch()
    );
    assert_eq!(
        actual.layout_fingerprint_patch().checked_offset(),
        actual.descriptor_definition_patch().checked_offset() + 32
    );
}
