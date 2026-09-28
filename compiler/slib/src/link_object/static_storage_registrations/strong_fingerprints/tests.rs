use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_static_storage_definition_fingerprints_v1,
    compute_strong_static_storage_registration_object_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1,
    verify_strong_static_storage_registrations_v1,
};

#[test]
fn computes_the_canonical_static_storage_strong_registration_fingerprint() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let shapes = shape_fingerprints(&fixture, &objects);

    let fingerprints =
        compute_strong_static_storage_fingerprints_v1(shapes, &fixture.canonical_shapes).unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    assert_eq!(actual.storage(), plan.semantic().storage());
    assert_eq!(
        actual.registration_node(),
        plan.registration_fingerprint_node()
    );
    assert_eq!(
        actual.registration_object_node(),
        plan.registration_object_node()
    );
    assert_eq!(
        actual.storage_definition_node(),
        plan.storage_definition_node()
    );
    assert_eq!(actual.layout_node(), plan.layout_fingerprint_node());
    assert_eq!(actual.scan_node(), plan.scan_fingerprint_node());
    assert_eq!(
        actual.registration().to_string(),
        "c1785b67ea1557c79afa65730546dcc574d9c270cc9d6ef9a7b7f1dd23403003"
    );
}

#[test]
fn commits_every_typed_direct_input_digest() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let shapes = shape_fingerprints(&fixture, &objects);
    let storage_definitions = shapes.storage_definitions();
    let registration_objects = storage_definitions.registration_objects();
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    let registration_object = registration_objects.fingerprints()[0];
    let storage_definition = storage_definitions.fingerprints()[0];
    let shape = shapes.fingerprints()[0];
    let compute = |registration_object_digest, storage_definition_digest, layout, scan| {
        strong_static_storage_fingerprint(
            plan,
            registration_object.node(),
            registration_object_digest,
            storage_definition.node(),
            storage_definition_digest,
            shape.layout_node(),
            layout,
            shape.scan_node(),
            scan,
        )
        .unwrap()
    };
    let baseline = compute(
        registration_object.fingerprint(),
        storage_definition.fingerprint(),
        shape.layout_fingerprint(),
        shape.scan_fingerprint(),
    );

    assert_ne!(
        baseline,
        compute(
            ObjectDefinitionFingerprintV1::from_array([1; 32]),
            storage_definition.fingerprint(),
            shape.layout_fingerprint(),
            shape.scan_fingerprint(),
        )
    );
    assert_ne!(
        baseline,
        compute(
            registration_object.fingerprint(),
            ObjectDefinitionFingerprintV1::from_array([2; 32]),
            shape.layout_fingerprint(),
            shape.scan_fingerprint(),
        )
    );
    assert_ne!(
        baseline,
        compute(
            registration_object.fingerprint(),
            storage_definition.fingerprint(),
            LayoutFingerprintV1([3; 32]),
            shape.scan_fingerprint(),
        )
    );
    assert_ne!(
        baseline,
        compute(
            registration_object.fingerprint(),
            storage_definition.fingerprint(),
            shape.layout_fingerprint(),
            ScanFingerprintV1::from_array([4; 32]),
        )
    );
}

#[test]
fn initial_state_protocol_changes_the_strong_record() {
    let encoded = fingerprint(Corruption::None);
    let zeroed = fingerprint(Corruption::StaticZeroedInitialState);
    let encoded_empty = fingerprint(Corruption::StaticEncodedEmptyInitialState);

    assert_ne!(encoded, zeroed);
    assert_ne!(encoded, encoded_empty);
    assert_ne!(zeroed, encoded_empty);
}

fn fingerprint(corruption: Corruption) -> RegistrationFingerprintV1 {
    let fixture = Fixture::new(corruption);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    compute_strong_static_storage_fingerprints_v1(
        shape_fingerprints(&fixture, &objects),
        &fixture.canonical_shapes,
    )
    .unwrap()
    .fingerprints()[0]
        .registration()
}

fn shape_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongStaticStorageShapeFingerprintSetV1 {
    let registrations = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_static_storage_registration_object_fingerprints_v1(registrations, objects)
            .unwrap();
    let storage_definitions =
        compute_strong_static_storage_definition_fingerprints_v1(registration_objects, objects)
            .unwrap();
    compute_strong_static_storage_shape_fingerprints_v1(storage_definitions).unwrap()
}
