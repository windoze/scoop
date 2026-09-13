use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, ScoopLirObjectCandidateV1,
    VerifiedStrongCallableFingerprintSetV1, VerifiedStrongTypeDependencyFingerprintSetV1,
    VerifiedStrongTypeDependencyFingerprintV1, VerifiedStrongTypeFingerprintSetV1,
    compute_strong_callable_body_object_fingerprints_v1, compute_strong_callable_fingerprints_v1,
    compute_strong_callable_registration_object_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1, compute_strong_type_fingerprints_v1,
    compute_strong_type_registration_object_fingerprints_v1,
    verify_scoop_lir_digest_patch_sites_v1, verify_strong_callable_registrations_v1,
    verify_strong_safepoint_registrations_v1, verify_strong_type_registrations_v1,
};
use scoop_lir::{DigestNodeV1, StrongDigestFinalizationPlanV1};

#[test]
fn writes_every_verified_registration_slot_and_revalidates_the_final_object() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let (safepoints, callables, types) = verified_fingerprints(&fixture, &objects);

    let patched =
        patch_strong_registration_fingerprints_v1(safepoints, callables, types, &objects).unwrap();

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

    let callable_registrations = patched
        .callables()
        .body_objects()
        .registration_objects()
        .registrations()
        .registrations();
    let callable_fingerprints = patched.callables().fingerprints();
    for (registration, fingerprint) in callable_registrations.iter().zip(callable_fingerprints) {
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
                registration.body_definition_patch().checked_offset(),
            ),
            fingerprint.body_definition().as_array()
        );
    }

    let type_registrations = patched
        .types()
        .dependencies()
        .registration_objects()
        .registrations()
        .registrations();
    let type_fingerprints = patched.types().fingerprints();
    for (registration, fingerprint) in type_registrations.iter().zip(type_fingerprints) {
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

    for (offset, (before, after)) in fixture
        .object_bytes
        .iter()
        .zip(final_object.bytes())
        .enumerate()
    {
        let offset = u64::try_from(offset).unwrap();
        let is_safepoint_patch = safepoint_registrations.iter().any(|registration| {
            [
                registration
                    .registration_definition_patch()
                    .checked_offset(),
                registration.normalized_stackmap_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        let is_callable_patch = callable_registrations.iter().any(|registration| {
            [
                registration
                    .registration_definition_patch()
                    .checked_offset(),
                registration.body_definition_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        let is_type_patch = type_registrations.iter().any(|registration| {
            [
                registration
                    .registration_definition_patch()
                    .checked_offset(),
                registration.descriptor_definition_patch().checked_offset(),
                registration.layout_fingerprint_patch().checked_offset(),
            ]
            .into_iter()
            .any(|start| start <= offset && offset < start + 32)
        });
        if !is_safepoint_patch && !is_callable_patch && !is_type_patch {
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
    let (safepoints, callables, types) = verified_fingerprints(&fixture, &objects);
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        patch_strong_registration_fingerprints_v1(safepoints, callables, types, &changed),
        Err(StrongRegistrationPatchError::ObjectValidation(
            StrongSafepointRegistrationValidationError::ObjectBytesMismatch(member)
        )) if member == fixture.member
    ));
}

#[test]
fn rejects_fingerprint_proofs_from_different_digest_graphs() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let safepoint_registrations = verify_strong_safepoint_registrations_v1(
        stackmaps.clone(),
        fixture.verified_patch_sites(),
        fixture.registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let safepoints =
        compute_strong_safepoint_fingerprints_v1(safepoint_registrations, &objects).unwrap();
    let types = verified_type_fingerprints(&fixture, &objects);

    let changed_node = fixture.registration_plan.registrations()[0].registration_fingerprint_node();
    let nodes = fixture
        .digest_plan
        .nodes()
        .iter()
        .map(|node| {
            if node.id() != changed_node {
                return node.clone();
            }
            DigestNodeV1::new(
                *node.key(),
                node.direct_inputs()[1..].to_vec(),
                node.patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let changed_digest_plan =
        StrongDigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
    let changed_patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        changed_digest_plan,
        &objects,
        &fixture.provisional_patch_sites,
    )
    .unwrap();
    let callable_registrations = verify_strong_callable_registrations_v1(
        changed_patch_sites,
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects = compute_strong_callable_registration_object_fingerprints_v1(
        callable_registrations,
        &objects,
    )
    .unwrap();
    let body_objects = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();
    let callables = compute_strong_callable_fingerprints_v1(body_objects).unwrap();

    assert_eq!(
        patch_strong_registration_fingerprints_v1(safepoints, callables, types, &objects),
        Err(StrongRegistrationPatchError::ProofMismatch)
    );
}

fn verified_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> (
    VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongTypeFingerprintSetV1,
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
    let safepoints =
        compute_strong_safepoint_fingerprints_v1(safepoint_registrations, objects).unwrap();
    let callable_registrations = verify_strong_callable_registrations_v1(
        patch_sites,
        fixture.callable_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects = compute_strong_callable_registration_object_fingerprints_v1(
        callable_registrations,
        objects,
    )
    .unwrap();
    let body_objects = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        objects,
    )
    .unwrap();
    let callables = compute_strong_callable_fingerprints_v1(body_objects).unwrap();
    let types = verified_type_fingerprints(fixture, objects);
    (safepoints, callables, types)
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
    let registration_objects =
        compute_strong_type_registration_object_fingerprints_v1(registrations, objects).unwrap();
    let plan = fixture.type_registration_plan.registrations()[0];
    compute_strong_type_fingerprints_v1(VerifiedStrongTypeDependencyFingerprintSetV1 {
        registration_objects,
        fingerprints: vec![VerifiedStrongTypeDependencyFingerprintV1 {
            exact_type: plan.exact_type(),
            descriptor_definition_node: plan.descriptor_definition_node(),
            descriptor_definition: ObjectDefinitionFingerprintV1::from_array([7; 32]),
            layout_node: plan.layout_fingerprint_node(),
            layout: LayoutFingerprintV1([11; 32]),
        }],
    })
    .unwrap()
}

fn digest_at(bytes: &[u8], offset: u64) -> &[u8] {
    let start = usize::try_from(offset).unwrap();
    &bytes[start..start + DIGEST_WIDTH]
}
