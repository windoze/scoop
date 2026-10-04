use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, StrongCallableBodyFingerprintError,
    compute_strong_callable_body_object_fingerprints_v1, verify_strong_callable_registrations_v1,
};

#[test]
fn computes_the_relocation_aware_registration_object_leaf() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    let bodies = compute_strong_callable_body_object_fingerprints_v1(
        registrations,
        fixture.verified_stackmaps(),
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();
    let fingerprints = bodies.registration_objects();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.callable_registration_plan.registrations()[0];
    assert_eq!(actual.body(), plan.body());
    assert_eq!(actual.node(), plan.registration_object_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "c137f8dca7abe722c5d750b7865a8e2ac69ef6e9d56c094cc78fc2ab271df259"
    );
}

#[test]
fn rechecks_object_bytes_before_hashing() {
    let mut fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let stackmaps = fixture.verified_stackmaps();
    let requirements = fixture.undefined_requirements();
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_callable_body_object_fingerprints_v1(
            registrations,
            stackmaps,
            requirements,
            &changed,
        ),
        Err(StrongCallableBodyFingerprintError::ObjectValidation(
            StrongCallableRegistrationValidationError::ObjectBytesMismatch(fixture.member)
        ))
    );
}
