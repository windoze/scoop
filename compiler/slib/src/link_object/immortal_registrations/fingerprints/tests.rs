use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, verify_strong_immortal_object_registrations_v1,
};

#[test]
fn computes_the_two_relocation_registration_object_leaf() {
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

    let fingerprints =
        compute_strong_immortal_object_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.immortal_registration_plan.registrations()[0];
    assert_eq!(actual.object(), plan.object());
    assert_eq!(actual.node(), plan.registration_object_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "861624621e2d2c7e7bad3e2f4c243c693e9e1deeede810791fabd4f5d69e5733"
    );
}

#[test]
fn encodes_a_core_type_registration_as_a_core_requirement() {
    let fixture = Fixture::new(Corruption::ExternalImmortalTypeRegistration);
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

    let fingerprints =
        compute_strong_immortal_object_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();

    assert_eq!(
        fingerprints.fingerprints()[0].fingerprint().to_string(),
        "3105450097f215c3b15eda9e83401288666df92d62bd0bd92b628a4fb49717d5"
    );
}

#[test]
fn rechecks_object_bytes_before_hashing() {
    let mut fixture = Fixture::new(Corruption::None);
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
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_immortal_object_registration_object_fingerprints_v1(registrations, &changed,),
        Err(
            StrongImmortalObjectRegistrationObjectFingerprintError::ObjectValidation(
                StrongImmortalObjectRegistrationValidationError::ObjectBytesMismatch(
                    fixture.member
                )
            )
        )
    );
}
