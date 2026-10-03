use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{ScoopLirObjectCandidateV1, verify_strong_safepoint_registrations_v1};

#[test]
fn computes_canonical_object_stackmap_and_strong_registration_fingerprints() {
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

    assert_eq!(fingerprints.fingerprints().len(), 2);
    assert!(
        fingerprints
            .fingerprints()
            .windows(2)
            .all(|pair| pair[0].site() < pair[1].site())
    );
    let actual = fingerprints
        .fingerprints()
        .iter()
        .map(|value| {
            (
                value.object_definition().to_string(),
                value.registration().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            (
                "4ebc6295a9922196f3265602817714e58ce4e7689440bb1687c8d2178ec37acc".to_owned(),
                "7416ace7b05f2ee33ae597cd325d5e05ba64886358338f76d9e7e045c4f9042f".to_owned(),
            ),
            (
                "e36df509155736e887f77c27dc70b2aa498b6084ebcdb8d96902ffa82e145674".to_owned(),
                "6bc81cfe6546c9261e268dae8f97cca3849276e2604c07961b7b89f9271b7165".to_owned(),
            ),
        ]
    );
}

#[test]
fn strong_fingerprint_commits_both_typed_direct_input_digests() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.registration_plan.registrations()[0];
    let object_node =
        DigestNodeId::from_key(&DigestNodeKey::object_definition(plan.primary_atom())).unwrap();
    let object = [7; 32];
    let stackmap = [11; 32];
    let baseline = strong_registration_fingerprint(plan, object_node, &object, &stackmap).unwrap();

    let mut changed_object = object;
    changed_object[9] ^= 1;
    assert_ne!(
        strong_registration_fingerprint(plan, object_node, &changed_object, &stackmap).unwrap(),
        baseline
    );
    let mut changed_stackmap = stackmap;
    changed_stackmap[17] ^= 1;
    assert_ne!(
        strong_registration_fingerprint(plan, object_node, &object, &changed_stackmap).unwrap(),
        baseline
    );
}

#[test]
fn rechecks_object_bytes_before_computing_any_fingerprint() {
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
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_safepoint_fingerprints_v1(registrations, &changed),
        Err(StrongSafepointFingerprintError::ObjectValidation(
            StrongSafepointRegistrationValidationError::ObjectBytesMismatch(fixture.member)
        ))
    );
}
