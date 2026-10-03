use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, StrongTypeFingerprintError, StrongTypeRegistrationValidationError,
    compute_strong_type_fingerprints_v1, verify_strong_type_registrations_v1,
};

#[test]
fn computes_the_relocation_aware_registration_object_leaf() {
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
        actual.registration_object_node(),
        plan.registration_object_node()
    );
    assert_eq!(
        actual.registration_object().to_string(),
        "b356cbacf7d952d42b4381b36b4383250c28cbfafbc0c7044323e69bf713c776"
    );
}

#[test]
fn rechecks_object_bytes_before_hashing() {
    let mut fixture = Fixture::new(Corruption::None);
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
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_type_fingerprints_v1(
            registrations,
            &fixture.canonical_shapes,
            fixture.undefined_requirements(),
            &changed
        ),
        Err(StrongTypeFingerprintError::Objects(
            StrongTypeRegistrationValidationError::ObjectBytesMismatch(fixture.member)
        ))
    );
}
