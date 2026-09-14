use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_type_registration_object_fingerprints_v1,
    verify_strong_type_registrations_v1,
};

#[test]
fn computes_descriptor_and_managed_layout_from_the_closed_proof() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let dependencies = compute_strong_type_dependency_fingerprints_v1(
        registration_objects(&fixture, &objects),
        &objects,
    )
    .unwrap();

    assert_eq!(dependencies.fingerprints().len(), 1);
    let actual = dependencies.fingerprints()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(actual.exact_type(), plan.exact_type());
    assert_eq!(
        actual.descriptor_definition_node(),
        plan.descriptor_definition_node()
    );
    assert_eq!(actual.layout_node(), plan.layout_fingerprint_node());
    assert_eq!(
        actual.descriptor_definition().to_string(),
        "f62ddddb6e0e546de80eb5da965db1f2f8779eb1fbbdcbaf3e1984f58028e015"
    );
    assert_eq!(
        actual.layout().to_string(),
        "b6dd142a0dfb7e75cdccc12bfada31c96c06e70f2589ad2ad13015fd28aeddf8"
    );
}

#[test]
fn rejects_descriptor_scalars_that_disagree_with_the_lir_shape() {
    let fixture = Fixture::new(Corruption::TypeDescriptorScalar);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let exact_type = fixture.type_registration_plan.registrations()[0].exact_type();

    assert!(matches!(
        compute_strong_type_dependency_fingerprints_v1(
            registration_objects(&fixture, &objects),
            &objects,
        ),
        Err(StrongTypeDependencyFingerprintError::DescriptorByteMismatch {
            exact_type: actual,
            offset_within_descriptor: 16,
            ..
        }) if actual == exact_type
    ));
}

#[test]
fn rechecks_object_bytes_after_registration_object_hashing() {
    let mut fixture = Fixture::new(Corruption::None);
    let original_objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let registration_objects = registration_objects(&fixture, &original_objects);
    fixture.object_bytes[0] ^= 1;
    let changed_objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_type_dependency_fingerprints_v1(registration_objects, &changed_objects,),
        Err(StrongTypeDependencyFingerprintError::ObjectValidation(
            StrongTypeRegistrationValidationError::ObjectBytesMismatch(fixture.member)
        ))
    );
}

fn registration_objects(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
    let registrations = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        objects,
    )
    .unwrap();
    compute_strong_type_registration_object_fingerprints_v1(registrations, objects).unwrap()
}
