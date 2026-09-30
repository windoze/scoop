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
                "3da1c9eb7b71b39229809e167452d3c0aa6b9804892ce89632de80fdad74fcf4".to_owned(),
                "43e6a83e21b6ef98e9e9670e1cd91f178ca143e8901e54e88fbb39faabe3e5df".to_owned(),
            ),
            (
                "f522ab6f4e6f1f91c1616886a746fb85941ae53ded55054346b14d461f18f6cc".to_owned(),
                "1dc24bf7742cd2a0a7950690ff7b17fab096c090230c675d66e7bae1031fd98c".to_owned(),
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
