use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_type_dependency_fingerprints_v1,
    compute_strong_type_registration_object_fingerprints_v1, verify_strong_type_registrations_v1,
};

#[test]
fn computes_the_canonical_type_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let dependencies = dependencies(&fixture, &objects);

    let fingerprints = compute_strong_type_fingerprints_v1(dependencies).unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(actual.exact_type(), plan.exact_type());
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(
        actual.registration().to_string(),
        "be155b7a11b148d32147c9a0fbb16188e68cdce41a12dba6ad644c7cce7d75d0"
    );
}

#[test]
fn carries_both_calculated_dependency_fingerprints() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let dependencies = dependencies(&fixture, &objects);
    let dependency = dependencies.fingerprints()[0];
    let fingerprints = compute_strong_type_fingerprints_v1(dependencies).unwrap();
    let actual = fingerprints.fingerprints()[0];

    assert_eq!(
        actual.descriptor_definition(),
        dependency.descriptor_definition()
    );
    assert_eq!(actual.layout(), dependency.layout());
}

fn dependencies(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongTypeDependencyFingerprintSetV1 {
    let registrations = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_type_registration_object_fingerprints_v1(registrations, objects).unwrap();
    compute_strong_type_dependency_fingerprints_v1(registration_objects, objects).unwrap()
}
