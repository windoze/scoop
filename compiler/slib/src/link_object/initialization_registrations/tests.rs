mod support;

use super::*;
use crate::link_object::{ProvisionalDigestPatchSiteV1, verify_scoop_lir_digest_patch_sites_v1};
use support::{Corruption, Fixture};

#[test]
fn verifies_exact_eager_initialization_artifacts() {
    let fixture = Fixture::new(false, Corruption::None);
    let objects = fixture.objects();
    let verified = verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        &objects,
    )
    .unwrap();

    assert_eq!(verified.producer(), fixture.plan.producer());
    assert_eq!(verified.registrations().len(), 1);
    let registration = &verified.registrations()[0];
    assert_eq!(
        registration.unit(),
        fixture.plan.registrations()[0].semantic().unit()
    );
    assert_eq!(registration.cell_checked_offset() % 8, 0);
    assert_eq!(registration.checked_offset() % 8, 0);
    assert_eq!(
        registration
            .registration_diagnostic_relocation()
            .offset_within_atom(),
        128
    );
    assert_eq!(
        registration
            .registration_gateway_relocation()
            .unwrap()
            .offset_within_atom(),
        312
    );
    assert_eq!(
        registration
            .gateway_definition_patch()
            .unwrap()
            .checked_offset(),
        registration.checked_offset() + 280
    );
}

#[test]
fn verifies_lazy_initialization_without_gateway_artifacts() {
    let fixture = Fixture::new(true, Corruption::None);
    let objects = fixture.objects();
    let verified = verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        &objects,
    )
    .unwrap();
    let registration = &verified.registrations()[0];

    assert!(registration.registration_gateway_relocation().is_none());
    assert!(registration.gateway_definition_patch().is_none());
}

#[test]
fn rejects_noncanonical_initialization_bytes() {
    for (corruption, role) in [
        (Corruption::CellByte, InitializationArtifactRoleV1::Cell),
        (
            Corruption::OldAbiVersion,
            InitializationArtifactRoleV1::Registration,
        ),
        (
            Corruption::RegistrationByte,
            InitializationArtifactRoleV1::Registration,
        ),
    ] {
        let fixture = Fixture::new(false, corruption);
        let unit = fixture.plan.registrations()[0].semantic().unit();
        assert!(matches!(
            verify_strong_initialization_registrations_v1(
                fixture.verified_patch_sites(),
                fixture.plan.clone(),
                &fixture.objects(),
            ),
            Err(StrongInitializationRegistrationValidationError::RecordByteMismatch {
                unit: actual,
                role: actual_role,
                ..
            }) if actual == unit && actual_role == role
        ));
    }
}

#[test]
fn rejects_wrong_initialization_relocation_targets() {
    for (corruption, role) in [
        (
            Corruption::StorageRegistrationTarget,
            InitializationRelocationRoleV1::RegistrationStorage,
        ),
        (
            Corruption::GatewayTarget,
            InitializationRelocationRoleV1::RegistrationGateway,
        ),
    ] {
        let fixture = Fixture::new(false, corruption);
        let unit = fixture.plan.registrations()[0].semantic().unit();
        assert!(matches!(
            verify_strong_initialization_registrations_v1(
                fixture.verified_patch_sites(),
                fixture.plan.clone(),
                &fixture.objects(),
            ),
            Err(StrongInitializationRegistrationValidationError::RelocationMismatch {
                unit: actual,
                role: actual_role,
                kind: InitializationRelocationFailureV1::TargetDefinition,
            }) if actual == unit && actual_role == role
        ));
    }
}

#[test]
fn rejects_diagnostic_bytes_that_do_not_match_the_semantic_path() {
    let fixture = Fixture::new(false, Corruption::DiagnosticByte);
    let unit = fixture.plan.registrations()[0].semantic().unit();

    assert_eq!(
        verify_strong_initialization_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.plan.clone(),
            &fixture.objects(),
        ),
        Err(
            StrongInitializationRegistrationValidationError::RelocationMismatch {
                unit,
                role: InitializationRelocationRoleV1::RegistrationDiagnostic,
                kind: InitializationRelocationFailureV1::DiagnosticTarget,
            }
        )
    );
}

#[test]
fn rejects_gateway_digest_slot_at_the_wrong_field() {
    let fixture = Fixture::new(false, Corruption::None);
    let registration = &fixture.plan.registrations()[0];
    let gateway_intent = registration.schedule().gateway_definition_patch().unwrap();
    let provisional = fixture
        .provisional_patch_sites
        .iter()
        .map(|site| {
            ProvisionalDigestPatchSiteV1::new(
                site.intent(),
                site.member(),
                if site.intent() == gateway_intent {
                    site.checked_offset() - 224
                } else {
                    site.checked_offset()
                },
                site.width_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        fixture.digest_plan.clone(),
        &fixture.objects(),
        &provisional,
    )
    .unwrap();

    assert_eq!(
        verify_strong_initialization_registrations_v1(
            patch_sites,
            fixture.plan.clone(),
            &fixture.objects(),
        ),
        Err(
            StrongInitializationRegistrationValidationError::PatchMismatch {
                unit: registration.semantic().unit(),
                intent: gateway_intent,
                kind: InitializationRegistrationPatchFailureV1::OffsetWithinAtom,
            }
        )
    );
}

#[test]
fn computes_canonical_initialization_strong_fingerprints() {
    let eager = Fixture::new(false, Corruption::None);
    let eager_objects = eager.objects();
    let eager_callable_bodies = callable_body_fingerprints(&eager, &eager_objects);
    let eager_definitions = initialization_definition_fingerprints(&eager, &eager_objects);
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

    let lazy = Fixture::new(true, Corruption::None);
    let lazy_objects = lazy.objects();
    let lazy_callable_bodies = callable_body_fingerprints(&lazy, &lazy_objects);
    let lazy_definitions = initialization_definition_fingerprints(&lazy, &lazy_objects);
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

fn initialization_definition_fingerprints(
    fixture: &Fixture,
    objects: &[crate::link_object::ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongInitializationRegistrationSetV1 {
    verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        objects,
    )
    .unwrap()
}

fn callable_body_fingerprints(
    fixture: &Fixture,
    objects: &[crate::link_object::ScoopLirObjectCandidateV1<'_>],
) -> crate::link_object::VerifiedStrongCallableBodyObjectFingerprintSetV1 {
    let registrations = crate::link_object::verify_strong_callable_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.callable_plan.clone(),
        objects,
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
