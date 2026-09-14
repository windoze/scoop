use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_type_registration_object_fingerprints_v1,
    verify_strong_type_registrations_v1,
};

#[test]
fn computes_the_canonical_type_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let dependencies = dependencies(&fixture, &objects, [7; 32], [11; 32]);

    let fingerprints = compute_strong_type_fingerprints_v1(dependencies).unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(actual.exact_type(), plan.exact_type());
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(actual.descriptor_definition().as_array(), &[7; 32]);
    assert_eq!(actual.layout().as_array(), &[11; 32]);
    assert_eq!(
        actual.registration().to_string(),
        "78ce6e19cf338e4a9d098e0755f0154ba5870d076bd6f00f3971fe7ab674a3ae"
    );
}

#[test]
fn commits_both_typed_dependency_fingerprints() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let baseline =
        compute_strong_type_fingerprints_v1(dependencies(&fixture, &objects, [7; 32], [11; 32]))
            .unwrap()
            .fingerprints()[0]
            .registration();
    let descriptor_changed =
        compute_strong_type_fingerprints_v1(dependencies(&fixture, &objects, [8; 32], [11; 32]))
            .unwrap()
            .fingerprints()[0]
            .registration();
    let layout_changed =
        compute_strong_type_fingerprints_v1(dependencies(&fixture, &objects, [7; 32], [12; 32]))
            .unwrap()
            .fingerprints()[0]
            .registration();

    assert_ne!(baseline, descriptor_changed);
    assert_ne!(baseline, layout_changed);
}

#[test]
fn rejects_dependency_proofs_bound_to_other_digest_nodes() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let plan = &fixture.type_registration_plan.registrations()[0];
    let exact_type = plan.exact_type();
    let mut descriptor = dependencies(&fixture, &objects, [7; 32], [11; 32]);
    descriptor.fingerprints[0].descriptor_definition_node = plan.registration_fingerprint_node();
    assert_eq!(
        compute_strong_type_fingerprints_v1(descriptor),
        Err(StrongTypeFingerprintError::DescriptorDefinitionMismatch { exact_type })
    );

    let mut layout = dependencies(&fixture, &objects, [7; 32], [11; 32]);
    layout.fingerprints[0].layout_node = plan.descriptor_definition_node();
    assert_eq!(
        compute_strong_type_fingerprints_v1(layout),
        Err(StrongTypeFingerprintError::LayoutMismatch { exact_type })
    );
}

fn dependencies(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
    descriptor: [u8; 32],
    layout: [u8; 32],
) -> VerifiedStrongTypeDependencyFingerprintSetV1 {
    let registrations = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_type_registration_object_fingerprints_v1(registrations, objects).unwrap();
    let plan = &fixture.type_registration_plan.registrations()[0];
    VerifiedStrongTypeDependencyFingerprintSetV1 {
        registration_objects,
        fingerprints: vec![VerifiedStrongTypeDependencyFingerprintV1 {
            exact_type: plan.exact_type(),
            descriptor_definition_node: plan.descriptor_definition_node(),
            descriptor_definition: ObjectDefinitionFingerprintV1::from_array(descriptor),
            layout_node: plan.layout_fingerprint_node(),
            layout: LayoutFingerprintV1(layout),
        }],
    }
}
