use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_immortal_object_definition_fingerprints_v1,
    compute_strong_immortal_object_registration_object_fingerprints_v1,
    verify_strong_immortal_object_registrations_v1,
};

#[test]
fn computes_the_canonical_immortal_object_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_immortal_object_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.immortal_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_immortal_object_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();
    let object_definitions = compute_strong_immortal_object_definition_fingerprints_v1(
        registration_objects,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();

    let fingerprints = compute_strong_immortal_object_fingerprints_v1(object_definitions).unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.immortal_registration_plan.registrations()[0];
    assert_eq!(actual.object(), plan.object());
    assert_eq!(
        actual.registration_object_node(),
        plan.registration_object_node()
    );
    assert_eq!(
        actual.object_definition_node(),
        plan.object_definition_node()
    );
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(
        actual.registration().to_string(),
        "9ef267260ceaf79a143109b2b25124f64c495afb7aaf9c77111fb4b159762380"
    );
}

#[test]
fn strong_fingerprint_commits_both_typed_object_definition_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.immortal_registration_plan.registrations()[0];
    let registration_object = ObjectDefinitionFingerprintV1::from_array([7; 32]);
    let object_definition = ObjectDefinitionFingerprintV1::from_array([11; 32]);
    let baseline = strong_immortal_object_registration_fingerprint(
        plan,
        plan.registration_object_node(),
        registration_object,
        plan.object_definition_node(),
        object_definition,
    )
    .unwrap();

    let mut changed_registration = *registration_object.as_array();
    changed_registration[9] ^= 1;
    assert_ne!(
        strong_immortal_object_registration_fingerprint(
            plan,
            plan.registration_object_node(),
            ObjectDefinitionFingerprintV1::from_array(changed_registration),
            plan.object_definition_node(),
            object_definition,
        )
        .unwrap(),
        baseline
    );
    let mut changed_object = *object_definition.as_array();
    changed_object[17] ^= 1;
    assert_ne!(
        strong_immortal_object_registration_fingerprint(
            plan,
            plan.registration_object_node(),
            registration_object,
            plan.object_definition_node(),
            ObjectDefinitionFingerprintV1::from_array(changed_object),
        )
        .unwrap(),
        baseline
    );
}
