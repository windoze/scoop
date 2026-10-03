use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{ScoopLirObjectCandidateV1, verify_strong_type_registrations_v1};

#[test]
fn computes_the_canonical_type_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let fingerprints = compute_strong_type_fingerprints_v1(
        registrations,
        &fixture.canonical_shapes,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(actual.exact_type(), plan.exact_type());
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(
        actual.registration().to_string(),
        "1efe9730d29d2bb403081780dfbaee80ec7f57d04ce38222cc6c4376a91a8841"
    );
}
