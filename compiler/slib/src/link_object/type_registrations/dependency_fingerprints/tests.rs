use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, StrongTypeFingerprintError, VerifiedStrongTypeFingerprintSetV1,
    compute_strong_type_fingerprints_v1, verify_strong_type_registrations_v1,
};

#[test]
fn computes_descriptor_and_managed_layout_from_the_closed_proof() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let dependencies = fingerprints(&fixture, &objects).unwrap();

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
        "a2bcd9f3ee79e23317abb39c6938c1e5743f6498fee7fa35a7953d3b954e1577"
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
        fingerprints(&fixture, &objects),
        Err(StrongTypeFingerprintError::Dependencies(StrongTypeDependencyFingerprintError::DescriptorByteMismatch {
            exact_type: actual,
            offset_within_descriptor: 16,
            ..
        })) if actual == exact_type
    ));
}

fn fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<VerifiedStrongTypeFingerprintSetV1, StrongTypeFingerprintError> {
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
}
