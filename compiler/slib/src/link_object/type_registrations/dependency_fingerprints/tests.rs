use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, StrongTypeFingerprintError, TypeFingerprintSetV1,
    bind_type_registrations_v1, compute_strong_type_fingerprints_v1,
};

#[test]
fn computes_descriptor_and_managed_layout_from_object_atoms_and_the_plan() {
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
        "29dae97edeb36bef6d7a94657558134d87c7622092dbc896131cfca2c2365a53"
    );
    assert_eq!(
        actual.layout().to_string(),
        "b6dd142a0dfb7e75cdccc12bfada31c96c06e70f2589ad2ad13015fd28aeddf8"
    );
}

fn fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<TypeFingerprintSetV1, StrongTypeFingerprintError> {
    let registrations = bind_type_registrations_v1(
        fixture.object_data(),
        fixture.type_registration_plan.clone(),
    );
    compute_strong_type_fingerprints_v1(registrations, &fixture.canonical_shapes, objects)
}
