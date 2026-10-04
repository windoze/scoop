use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_callable_body_object_fingerprints_v1,
    verify_strong_callable_registrations_v1,
};

#[test]
fn computes_the_canonical_callable_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let registrations = verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects = registrations;
    let body_objects = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();

    let fingerprints =
        compute_strong_callable_fingerprints_v1(body_objects, &fixture.canonical_callables)
            .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.callable_registration_plan.registrations()[0];
    assert_eq!(actual.body(), plan.body());
    assert_eq!(
        actual.registration_object_node(),
        plan.registration_object_node()
    );
    assert_eq!(actual.body_definition_node(), plan.body_definition_node());
    assert_eq!(
        actual.definition(),
        CallableDefinitionFingerprintV1::Strong(actual.body_definition())
    );
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(
        actual.registration().to_string(),
        "f5f40103e6b87acff4d18c82135d364d5f1ea85b4c5d47b557273abe61f591cc"
    );
}

#[test]
fn strong_fingerprint_commits_both_typed_object_definition_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.callable_registration_plan.registrations()[0];
    let registration_object = ObjectDefinitionFingerprintV1::from_array([7; 32]);
    let body_definition = ObjectDefinitionFingerprintV1::from_array([11; 32]);
    let baseline = strong_callable_registration_fingerprint(
        plan,
        plan.registration_object_node(),
        registration_object,
        plan.body_definition_node(),
        body_definition,
    )
    .unwrap();

    let mut changed_registration = *registration_object.as_array();
    changed_registration[9] ^= 1;
    assert_ne!(
        strong_callable_registration_fingerprint(
            plan,
            plan.registration_object_node(),
            ObjectDefinitionFingerprintV1::from_array(changed_registration),
            plan.body_definition_node(),
            body_definition,
        )
        .unwrap(),
        baseline
    );
    let mut changed_body = *body_definition.as_array();
    changed_body[17] ^= 1;
    assert_ne!(
        strong_callable_registration_fingerprint(
            plan,
            plan.registration_object_node(),
            registration_object,
            plan.body_definition_node(),
            ObjectDefinitionFingerprintV1::from_array(changed_body),
        )
        .unwrap(),
        baseline
    );
}
