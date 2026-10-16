mod support;
use super::*;
use support::Fixture;

#[test]
fn binds_eager_gateway_and_omits_lazy_gateway() {
    for lazy in [false, true] {
        let fixture = Fixture::new(lazy);
        let data = bind_initialization_registrations_v1(
            &fixture.verified_patch_sites(),
            fixture.plan.clone(),
        )
        .unwrap();
        assert_eq!(data.registrations().len(), 1);
        let actual = &data.registrations()[0];
        let plan = &fixture.plan.registrations()[0];
        assert_eq!(actual.unit(), plan.semantic().unit());
        assert_eq!(
            actual
                .gateway_definition_patch()
                .map(|patch| patch.intent()),
            plan.schedule().gateway_definition_patch()
        );
        assert_eq!(actual.gateway_definition_patch().is_none(), lazy);
    }
}

#[test]
fn computes_canonical_initialization_strong_fingerprints() {
    let eager = Fixture::new(false);
    let eager_objects = eager.objects();
    let eager_callable_bodies = callable_body_fingerprints(&eager, &eager_objects);
    let eager_definitions = initialization_data(&eager);
    let eager_fingerprints = compute_strong_initialization_fingerprints_v1(
        eager_definitions,
        &eager_callable_bodies,
        &scoop_lir::CanonicalShapeAbisV1::new(Vec::new(), &eager.foundation).unwrap(),
    )
    .unwrap();

    let eager_actual = eager_fingerprints.fingerprints()[0];
    let eager_plan = &eager.plan.registrations()[0];
    let eager_gateway = eager_plan.schedule().gateway().unwrap();
    assert_eq!(eager_actual.unit(), eager_plan.semantic().unit());
    assert_eq!(eager_actual.gateway_body(), Some(eager_gateway.body()));
    assert_eq!(
        eager_actual.gateway_definition_node(),
        Some(eager_gateway.body_definition_node())
    );
    let eager_patched =
        crate::link_object::strong_registration_finalization::patch_initializations_for_test(
            &eager_fingerprints,
            &eager_objects,
        )
        .unwrap();
    let eager_verified = &eager_fingerprints.registrations().registrations()[0];
    assert_eq!(
        digest_at(
            &eager_patched[0].1,
            eager_verified
                .gateway_definition_patch()
                .unwrap()
                .checked_offset()
        ),
        eager_actual.gateway_definition().unwrap().as_array()
    );

    let lazy = Fixture::new(true);
    let lazy_objects = lazy.objects();
    let lazy_callable_bodies = callable_body_fingerprints(&lazy, &lazy_objects);
    let lazy_definitions = initialization_data(&lazy);
    let lazy_fingerprints = compute_strong_initialization_fingerprints_v1(
        lazy_definitions,
        &lazy_callable_bodies,
        &scoop_lir::CanonicalShapeAbisV1::new(Vec::new(), &lazy.foundation).unwrap(),
    )
    .unwrap();
    let lazy_actual = lazy_fingerprints.fingerprints()[0];
    assert_eq!(lazy_actual.gateway_body(), None);
    assert_eq!(lazy_actual.gateway_definition_node(), None);
    assert_eq!(lazy_actual.gateway_definition(), None);
    assert_eq!(
        eager_actual.registration(),
        crate::RegistrationAbiV1::Strong
    );
    assert_eq!(lazy_actual.registration(), crate::RegistrationAbiV1::Strong);
    let lazy_patched =
        crate::link_object::strong_registration_finalization::patch_initializations_for_test(
            &lazy_fingerprints,
            &lazy_objects,
        )
        .unwrap();
    assert_eq!(lazy_patched[0].1, lazy.object_bytes);
    let lazy_verified = &lazy_fingerprints.registrations().registrations()[0];
    assert!(lazy_verified.gateway_definition_patch().is_none());
}

fn digest_at(bytes: &[u8], offset: u64) -> &[u8] {
    let start = usize::try_from(offset).unwrap();
    &bytes[start..start + 32]
}

fn initialization_data(fixture: &Fixture) -> InitializationRegistrationDataV1 {
    bind_initialization_registrations_v1(&fixture.verified_patch_sites(), fixture.plan.clone())
        .unwrap()
}

fn callable_body_fingerprints(
    fixture: &Fixture,
    objects: &[crate::link_object::ScoopLirObjectCandidateV1<'_>],
) -> crate::link_object::VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    let registrations = crate::link_object::bind_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_plan.clone(),
    )
    .unwrap();
    let registration_objects = registrations;
    crate::link_object::compute_strong_callable_body_object_fingerprints_v1(
        registration_objects,
        fixture.verified_stackmaps(),
        fixture.undefined_requirements(),
        objects,
    )
    .unwrap()
}
