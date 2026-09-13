use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_callable_registration_object_fingerprints_v1,
    verify_scoop_lir_digest_patch_sites_v1, verify_strong_callable_registrations_v1,
};
use scoop_lir::{DigestNodeV1, StrongDigestFinalizationPlanV1};

#[test]
fn hashes_normalized_body_code_relocations_and_stackmap_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let registrations = verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_callable_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();

    let fingerprints = compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        stackmaps,
        fixture.undefined_requirements(),
        &objects,
    )
    .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.callable_registration_plan.registrations()[0];
    assert_eq!(actual.body(), plan.body());
    assert_eq!(actual.node(), plan.body_definition_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "71400e1b1ac2394499c186b901232f8954b79b20d799f1fd8d225025356e902a"
    );
}

#[test]
fn rechecks_body_object_bytes_before_hashing() {
    let mut fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let registrations = verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_callable_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_callable_body_object_fingerprints_v1(
            registration_objects,
            stackmaps,
            fixture.undefined_requirements(),
            &changed,
        ),
        Err(StrongCallableBodyFingerprintError::ObjectValidation(
            StrongCallableRegistrationValidationError::ObjectBytesMismatch(fixture.member)
        ))
    );
}

#[test]
fn rejects_body_digest_nodes_without_the_exact_stackmap_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.callable_registration_plan.registrations()[0];
    let nodes = fixture
        .digest_plan
        .nodes()
        .iter()
        .map(|node| {
            if node.id() != plan.body_definition_node() {
                return node.clone();
            }
            DigestNodeV1::new(
                *node.key(),
                Vec::new(),
                node.patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let wrong_digest_plan =
        StrongDigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        wrong_digest_plan,
        &objects,
        &fixture.provisional_patch_sites,
    )
    .unwrap();
    let registrations = verify_strong_callable_registrations_v1(
        patch_sites,
        fixture.callable_registration_plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_callable_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();

    assert_eq!(
        compute_strong_callable_body_object_fingerprints_v1(
            registration_objects,
            stackmaps,
            fixture.undefined_requirements(),
            &objects,
        ),
        Err(StrongCallableBodyFingerprintError::BodyDirectInputMismatch { body: plan.body() })
    );
}
