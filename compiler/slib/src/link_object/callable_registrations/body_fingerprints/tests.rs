use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{ScoopLirObjectCandidateV1, bind_callable_registrations_v1};

#[test]
fn hashes_normalized_body_code_relocations_and_stackmap_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let registrations = bind_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
    )
    .unwrap();
    let registration_objects = registrations;

    let fingerprints = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.callable_registration_plan.registrations()[0];
    assert_eq!(actual.body(), plan.body());
    assert_eq!(actual.node(), plan.body_definition_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "ae9aee8c0014562fb385854bca3b2427df30ad417643db80ae2bdaa327c53330"
    );
}
