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
    assert_eq!(
        fingerprints.fingerprints()[0]
            .object_definition()
            .to_string(),
        "1ad6e16427b8a13702aa9d328c8afa3582fa3853a914d3c9c70250211d625fd1"
    );
    assert_eq!(
        fingerprints.fingerprints()[0].registration().to_string(),
        "fdf327d242b769fe3f25348e2d4cd9bc05f6457835898b62e37d1ee504742578"
    );
    assert_eq!(
        fingerprints.fingerprints()[1]
            .object_definition()
            .to_string(),
        "d932ab5be26400e0936b8e3632daf25723f2a87090b080760edb65b3e1cf7351"
    );
    assert_eq!(
        fingerprints.fingerprints()[1].registration().to_string(),
        "5b86ae4675046a8b1887268e048633b4ec688a23ab81455b096ec12a72e667ec"
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
