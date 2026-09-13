use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{ScoopLirObjectCandidateV1, verify_strong_callable_registrations_v1};

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

    let fingerprints =
        compute_strong_callable_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.callable_registration_plan.registrations()[0];
    assert_eq!(actual.body(), plan.body());
    assert_eq!(actual.node(), plan.registration_object_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "a4ab9830be48d095acc3c7f333e0825e26bbbf522c97b7b12fddb639ce23aff9"
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
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_callable_registration_object_fingerprints_v1(registrations, &changed),
        Err(
            StrongCallableRegistrationObjectFingerprintError::ObjectValidation(
                StrongCallableRegistrationValidationError::ObjectBytesMismatch(fixture.member)
            )
        )
    );
}
