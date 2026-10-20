use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ImmortalObjectFingerprintSetV1, InitializationFingerprintSetV1, ScoopLirObjectCandidateV1,
    StaticStorageFingerprintSetV1, TypeFingerprintSetV1, VerifiedStrongCallableFingerprintSetV1,
    bind_callable_registrations_v1, bind_safepoint_registrations_v1, bind_type_registrations_v1,
    compute_strong_callable_body_object_fingerprints_v1, compute_strong_callable_fingerprints_v1,
    compute_strong_immortal_object_fingerprints_v1, compute_strong_initialization_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1, compute_strong_static_storage_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1, compute_strong_type_fingerprints_v1,
};
use scoop_wire::sha256;

#[test]
fn preserves_original_object_bytes_and_reuses_their_content_digest() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let (safepoints, callables, types, immortal_objects, static_storages, initializations) =
        verified_fingerprints(&fixture, &objects);

    let collected = collect_registration_objects_v1(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        &objects,
    )
    .unwrap();

    assert_eq!(
        collected.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(collected.objects().len(), 1);
    let final_object = &collected.objects()[0];
    assert_eq!(final_object.member(), fixture.member);
    assert_eq!(
        final_object
            .envelope()
            .sections()
            .envelope()
            .content_digest(),
        sha256(final_object.bytes())
    );
    assert_eq!(final_object.bytes(), fixture.object_bytes);
}

fn verified_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> (
    VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongCallableFingerprintSetV1,
    TypeFingerprintSetV1,
    ImmortalObjectFingerprintSetV1,
    StaticStorageFingerprintSetV1,
    InitializationFingerprintSetV1,
) {
    let stackmaps = fixture.verified_stackmaps();

    let safepoint_registrations =
        bind_safepoint_registrations_v1(stackmaps.clone(), fixture.registration_plan.clone());
    let safepoints = compute_strong_safepoint_fingerprints_v1(safepoint_registrations).unwrap();
    let callable_registrations = bind_callable_registrations_v1(
        fixture.object_data(),
        fixture.callable_registration_plan.clone(),
    );
    let body_objects = compute_strong_callable_body_object_fingerprints_v1(
        callable_registrations,
        stackmaps,
        fixture.undefined_requirements(),
        objects,
    )
    .unwrap();
    let initializations = verified_initialization_fingerprints(fixture, &body_objects);
    let callables =
        compute_strong_callable_fingerprints_v1(body_objects, &fixture.canonical_callables)
            .unwrap();
    let types = verified_type_fingerprints(fixture, objects);
    let immortal_objects = immortal_object_fingerprints(fixture);
    let static_storages = static_storage_fingerprints(fixture);
    (
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
    )
}

fn verified_initialization_fingerprints(
    fixture: &Fixture,
    callable_bodies: &crate::link_object::VerifiedStrongCallableBodyObjectFingerprintSetV1,
) -> InitializationFingerprintSetV1 {
    let registrations = fixture.initialization_registration_plan.clone();

    compute_strong_initialization_fingerprints_v1(
        registrations,
        callable_bodies,
        &fixture.canonical_shapes,
    )
    .unwrap()
}

fn static_storage_fingerprints(fixture: &Fixture) -> StaticStorageFingerprintSetV1 {
    let registrations = fixture.static_storage_registration_plan.clone();
    let shapes = compute_strong_static_storage_shape_fingerprints_v1(registrations).unwrap();
    compute_strong_static_storage_fingerprints_v1(shapes, &fixture.canonical_shapes).unwrap()
}

fn immortal_object_fingerprints(fixture: &Fixture) -> ImmortalObjectFingerprintSetV1 {
    compute_strong_immortal_object_fingerprints_v1(
        fixture.immortal_registration_plan.clone(),
        &fixture.canonical_shapes,
    )
    .unwrap()
}

fn verified_type_fingerprints(
    fixture: &Fixture,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> TypeFingerprintSetV1 {
    let registrations = bind_type_registrations_v1(
        fixture.object_data(),
        fixture.type_registration_plan.clone(),
    );
    compute_strong_type_fingerprints_v1(registrations, &fixture.canonical_shapes, objects).unwrap()
}
