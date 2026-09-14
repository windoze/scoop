use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_static_storage_registration_object_fingerprints_v1,
    verify_strong_static_storage_registrations_v1,
};

#[test]
fn hashes_the_writable_storage_and_typed_immortal_relocation() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registration_objects = registration_objects(&fixture, &objects);

    let fingerprints =
        compute_strong_static_storage_definition_fingerprints_v1(registration_objects, &objects)
            .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    assert_eq!(actual.storage(), plan.semantic().storage());
    assert_eq!(actual.node(), plan.storage_definition_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "593d1351146ef3c5d53318545956ea7d030b84d0c669801c54ed0b9fe221dbcd"
    );
}

#[test]
fn hashes_physical_storage_independently_of_the_initialization_protocol() {
    let encoded = fingerprint(Corruption::None);
    let zeroed = fingerprint(Corruption::StaticZeroedInitialState);
    let encoded_empty = fingerprint(Corruption::StaticEncodedEmptyInitialState);

    assert_ne!(encoded, zeroed);
    assert_ne!(encoded, encoded_empty);
    assert_eq!(zeroed, encoded_empty);
}

#[test]
fn rechecks_object_bytes_after_registration_object_hashing() {
    let fixture = Fixture::new(Corruption::None);
    let original = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registration_objects = registration_objects(&fixture, &original);
    let mut changed = fixture.object_bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(fixture.member, &changed)];

    assert!(matches!(
        compute_strong_static_storage_definition_fingerprints_v1(
            registration_objects,
            &changed,
        ),
        Err(StrongStaticStorageDefinitionFingerprintError::ObjectValidation(
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
    let registration_objects = registration_objects(&fixture, &objects);
    compute_strong_static_storage_definition_fingerprints_v1(registration_objects, &objects)
        .unwrap()
        .fingerprints()[0]
        .fingerprint()
}

fn registration_objects(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        objects,
    )
    .unwrap();
    compute_strong_static_storage_registration_object_fingerprints_v1(verified, objects).unwrap()
}
