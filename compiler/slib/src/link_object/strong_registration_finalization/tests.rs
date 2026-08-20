use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongImmortalObjectFingerprintSetV1, VerifiedStrongInitializationFingerprintSetV1,
    VerifiedStrongStaticStorageFingerprintSetV1, VerifiedStrongTypeFingerprintSetV1,
    compute_strong_callable_body_object_fingerprints_v1, compute_strong_callable_fingerprints_v1,
    compute_strong_immortal_object_fingerprints_v1, compute_strong_initialization_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1, compute_strong_static_storage_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1, compute_strong_type_fingerprints_v1,
    verify_strong_callable_registrations_v1, verify_strong_immortal_object_registrations_v1,
    verify_strong_initialization_registrations_v1, verify_strong_safepoint_registrations_v1,
    verify_strong_static_storage_registrations_v1, verify_strong_type_registrations_v1,
};

#[test]
fn writes_every_verified_registration_slot_and_revalidates_the_final_object() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let (safepoints, callables, types, immortal_objects, static_storages, initializations) =
        verified_fingerprints(&fixture, &objects);

    let patched = patch_strong_registration_fingerprints_v1(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        &objects,
    )
    .unwrap();

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

    let safepoint_registrations = patched.safepoints().registrations().registrations();
    let safepoint_fingerprints = patched.safepoints().fingerprints();
    for (registration, fingerprint) in safepoint_registrations.iter().zip(safepoint_fingerprints) {
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.normalized_stackmap_patch().checked_offset(),
            ),
            fingerprint.stackmap().as_array()
        );
    }

    let callable_registrations = patched
        .callables()
        .body_objects()
        .registrations()
        .registrations();
    let callable_fingerprints = patched.callables().fingerprints();
    for (registration, fingerprint) in callable_registrations.iter().zip(callable_fingerprints) {
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.body_definition_patch().checked_offset(),
            ),
            fingerprint.body_definition().as_array()
        );
    }

    let type_registrations = patched.types().registrations().registrations();
    let type_fingerprints = patched.types().fingerprints();
    for (registration, fingerprint) in type_registrations.iter().zip(type_fingerprints) {
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.descriptor_definition_patch().checked_offset(),
            ),
            fingerprint.descriptor_definition().as_array()
        );
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.layout_fingerprint_patch().checked_offset(),
            ),
            fingerprint.layout().as_array()
        );
    }

    let static_storage_registrations = patched
        .static_storages()
        .shapes()
        .registrations()
        .registrations();
    let static_storage_fingerprints = patched.static_storages().fingerprints();
    for (registration, fingerprint) in static_storage_registrations
        .iter()
        .zip(static_storage_fingerprints)
    {
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.scan_fingerprint_patch().checked_offset(),
            ),
            fingerprint.scan().as_array()
        );
        assert_eq!(
            digest_at(
                final_object.bytes(),
                registration.layout_fingerprint_patch().checked_offset(),
            ),
            fingerprint.layout().as_array()
        );
    }

    for (offset, (before, after)) in fixture
        .object_bytes
        .iter()
        .zip(final_object.bytes())
        .enumerate()
    {
        let offset = u64::try_from(offset).unwrap();
        let is_safepoint_patch = safepoint_registrations.iter().any(|registration| {
            [registration.normalized_stackmap_patch().checked_offset()]
                .into_iter()
                .any(|start| start <= offset && offset < start + 32)
        });
        let is_callable_patch = callable_registrations.iter().any(|registration| {
            [registration.body_definition_patch().checked_offset()]
                .into_iter()
                .any(|start| start <= offset && offset < start + 32)
        });
        let is_type_patch = type_registrations.iter().any(|registration| {
            [
                registration.descriptor_definition_patch().checked_offset(),
                registration.layout_fingerprint_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        let is_static_storage_patch = static_storage_registrations.iter().any(|registration| {
            [
                registration.scan_fingerprint_patch().checked_offset(),
                registration.layout_fingerprint_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        if !is_safepoint_patch && !is_callable_patch && !is_type_patch && !is_static_storage_patch {
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
    let (safepoints, callables, types, immortal_objects, static_storages, initializations) =
        verified_fingerprints(&fixture, &objects);
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        patch_strong_registration_fingerprints_v1(
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            &changed,
        ),
        Err(StrongRegistrationPatchError::ObjectValidation(
            StrongSafepointRegistrationValidationError::ObjectBytesMismatch(member)
        )) if member == fixture.member
    ));
}

fn verified_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> (
    VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongTypeFingerprintSetV1,
    VerifiedStrongImmortalObjectFingerprintSetV1,
    VerifiedStrongStaticStorageFingerprintSetV1,
    VerifiedStrongInitializationFingerprintSetV1,
) {
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let safepoint_registrations = verify_strong_safepoint_registrations_v1(
        stackmaps.clone(),
        patch_sites.clone(),
        fixture.registration_plan.clone(),
        objects,
    )
    .unwrap();
    let safepoints = compute_strong_safepoint_fingerprints_v1(safepoint_registrations).unwrap();
    let callable_registrations = verify_strong_callable_registrations_v1(
        patch_sites,
        fixture.callable_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects = callable_registrations;
    let body_objects = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        objects,
    )
    .unwrap();
    let initializations = verified_initialization_fingerprints(
        fixture,
        objects,
        fixture.verified_patch_sites(),
        &body_objects,
    );
    let callables =
        compute_strong_callable_fingerprints_v1(body_objects, &fixture.canonical_callables)
            .unwrap();
    let types = verified_type_fingerprints(fixture, objects);
    let immortal_objects = verified_immortal_object_fingerprints(fixture, objects);
    let static_storages = verified_static_storage_fingerprints(fixture, objects);
    (
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
    )
}

fn verified_initialization_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
    patch_sites: crate::link_object::VerifiedScoopLirDigestPatchSiteSetV1,
    callable_bodies: &crate::link_object::VerifiedStrongCallableBodyObjectFingerprintSetV1,
) -> VerifiedStrongInitializationFingerprintSetV1 {
    let registrations = verify_strong_initialization_registrations_v1(
        patch_sites,
        fixture.initialization_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects = registrations;
    let definitions = registration_objects;
    compute_strong_initialization_fingerprints_v1(
        definitions,
        callable_bodies,
        &fixture.canonical_shapes,
    )
    .unwrap()
}

fn verified_static_storage_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongStaticStorageFingerprintSetV1 {
    let registrations = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects = registrations;
    let storage_definitions = registration_objects;
    let shapes = compute_strong_static_storage_shape_fingerprints_v1(storage_definitions).unwrap();
    compute_strong_static_storage_fingerprints_v1(shapes, &fixture.canonical_shapes).unwrap()
}

fn verified_immortal_object_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongImmortalObjectFingerprintSetV1 {
    let registrations = verify_strong_immortal_object_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.immortal_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects = registrations;
    let object_definitions = registration_objects;
    compute_strong_immortal_object_fingerprints_v1(object_definitions, &fixture.canonical_shapes)
        .unwrap()
}

fn verified_type_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongTypeFingerprintSetV1 {
    let registrations = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        objects,
    )
    .unwrap();
    compute_strong_type_fingerprints_v1(
        registrations,
        &fixture.canonical_shapes,
        fixture.undefined_requirements(),
        objects,
    )
    .unwrap()
}

fn digest_at(bytes: &[u8], offset: u64) -> &[u8] {
    let start = usize::try_from(offset).unwrap();
    &bytes[start..start + DIGEST_WIDTH]
}
