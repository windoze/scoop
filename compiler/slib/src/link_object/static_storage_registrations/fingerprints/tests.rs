use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, verify_strong_static_storage_registrations_v1,
};

#[test]
fn computes_the_canonical_static_registration_object_leaf() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    let fingerprints =
        compute_strong_static_storage_registration_object_fingerprints_v1(verified, &objects)
            .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    assert_eq!(actual.storage(), plan.semantic().storage());
    assert_eq!(actual.node(), plan.registration_object_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "0cbf8eb2f4421647ab7c208a2750d7dec836bdcb5d94499547af279043ef1816"
    );
}

#[test]
fn initial_state_target_kinds_change_the_object_leaf() {
    let encoded = fingerprint(Corruption::None);
    let zeroed = fingerprint(Corruption::StaticZeroedInitialState);
    let encoded_empty = fingerprint(Corruption::StaticEncodedEmptyInitialState);

    assert_ne!(encoded, zeroed);
    assert_ne!(encoded, encoded_empty);
    assert_ne!(zeroed, encoded_empty);
}

#[test]
fn rechecks_object_bytes_before_hashing() {
    let fixture = Fixture::new(Corruption::None);
    let original = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &original,
    )
    .unwrap();
    let mut changed = fixture.object_bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(fixture.member, &changed)];

    assert!(matches!(
        compute_strong_static_storage_registration_object_fingerprints_v1(verified, &changed),
        Err(StrongStaticStorageRegistrationObjectFingerprintError::ObjectValidation(
            StrongStaticStorageRegistrationValidationError::ObjectBytesMismatch(member)
        )) if member == fixture.member
    ));
}

fn fingerprint(corruption: Corruption) -> ObjectDefinitionFingerprintV1 {
    let fixture = Fixture::new(corruption);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    compute_strong_static_storage_registration_object_fingerprints_v1(verified, &objects)
        .unwrap()
        .fingerprints()[0]
        .fingerprint()
}
