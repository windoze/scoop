use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_safepoint_fingerprints_v1,
    verify_strong_safepoint_registrations_v1,
};

#[test]
fn writes_only_the_two_verified_digest_slots_and_revalidates_the_final_object() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_safepoint_registrations_v1(
        fixture.verified_stackmaps(),
        fixture.verified_patch_sites(),
        fixture.registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let fingerprints = compute_strong_safepoint_fingerprints_v1(registrations, &objects).unwrap();

    let patched = patch_strong_safepoint_fingerprints_v1(fingerprints, &objects).unwrap();

    assert_eq!(
        patched.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(patched.objects().len(), 1);
    let final_object = &patched.objects()[0];
    assert_eq!(final_object.member(), fixture.member);
    assert_eq!(
        final_object
            .envelope()
            .sections()
            .envelope()
            .content_digest(),
        sha256(final_object.bytes())
    );
    assert_ne!(
        final_object
            .envelope()
            .sections()
            .envelope()
            .content_digest(),
        sha256(&fixture.object_bytes)
    );
    let registrations = patched.fingerprints().registrations().registrations();
    let fingerprints = patched.fingerprints().fingerprints();
    for (registration, fingerprint) in registrations.iter().zip(fingerprints) {
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration
                    .registration_definition_patch()
                    .checked_offset(),
            ),
            fingerprint.registration().as_array()
        );
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.normalized_stackmap_patch().checked_offset(),
            ),
            fingerprint.stackmap().as_array()
        );
    }
    for (offset, (before, after)) in fixture
        .object_bytes
        .iter()
        .zip(final_object.bytes())
        .enumerate()
    {
        let offset = u64::try_from(offset).unwrap();
        let is_patch = registrations.iter().any(|registration| {
            [
                registration
                    .registration_definition_patch()
                    .checked_offset(),
                registration.normalized_stackmap_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        if !is_patch {
            assert_eq!(before, after);
        }
    }
}

#[test]
fn rejects_provisional_bytes_changed_after_fingerprint_computation() {
    let mut fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registrations = verify_strong_safepoint_registrations_v1(
        fixture.verified_stackmaps(),
        fixture.verified_patch_sites(),
        fixture.registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let fingerprints = compute_strong_safepoint_fingerprints_v1(registrations, &objects).unwrap();
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        patch_strong_safepoint_fingerprints_v1(fingerprints, &changed),
        Err(StrongSafepointPatchError::ObjectValidation(
            StrongSafepointRegistrationValidationError::ObjectBytesMismatch(member)
        )) if member == fixture.member
    ));
}

fn digest_at(bytes: &[u8], offset: u64) -> &[u8] {
    let start = usize::try_from(offset).unwrap();
    &bytes[start..start + DIGEST_WIDTH]
}
