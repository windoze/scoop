mod support;

use scoop_lir::{DigestFinalizationPlanV1, DigestNodeV1};

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
        160
    );
    assert_eq!(
        registration
            .registration_gateway_relocation()
            .unwrap()
            .offset_within_atom(),
        344
    );
    assert_eq!(
        registration
            .registration_definition_patch()
            .checked_offset(),
        registration.checked_offset() + 120
    );
    assert_eq!(
        registration
            .gateway_definition_patch()
            .unwrap()
            .checked_offset(),
        registration.checked_offset() + 312
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
fn rejects_registration_digest_graph_drift() {
    let fixture = Fixture::new(false, Corruption::None);
    let registration = &fixture.plan.registrations()[0];
    let nodes = fixture
        .digest_plan
        .nodes()
        .iter()
        .map(|node| {
            if node.id() != registration.registration_fingerprint_node() {
                return node.clone();
            }
            DigestNodeV1::new(
                *node.key(),
                node.direct_inputs()[1..].to_vec(),
                node.patch_intents()
                    .iter()
                    .map(|patch| *patch.key())
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let digest_plan = DigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        digest_plan,
        &fixture.objects(),
        &fixture.provisional_patch_sites,
    )
    .unwrap();

    assert_eq!(
        verify_strong_initialization_registrations_v1(
            patch_sites,
            fixture.plan.clone(),
            &fixture.objects(),
        ),
        Err(
            StrongInitializationRegistrationValidationError::DigestPlanMismatch {
                unit: registration.semantic().unit(),
                kind: InitializationRegistrationDigestPlanFailureV1::RegistrationDirectInputs,
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
                    site.checked_offset() - 256
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
fn computes_canonical_initialization_registration_object_leaves() {
    let eager = Fixture::new(false, Corruption::None);
    let eager_objects = eager.objects();
    let eager_verified = verify_strong_initialization_registrations_v1(
        eager.verified_patch_sites(),
        eager.plan.clone(),
        &eager_objects,
    )
    .unwrap();
    let eager_fingerprints = compute_strong_initialization_registration_object_fingerprints_v1(
        eager_verified,
        &eager_objects,
    )
    .unwrap();
    let eager_fingerprint = eager_fingerprints.fingerprints()[0];
    let eager_plan = &eager.plan.registrations()[0];
    assert_eq!(eager_fingerprint.unit(), eager_plan.semantic().unit());
    assert_eq!(
        eager_fingerprint.node(),
        eager_plan.registration_object_node()
    );
    assert_eq!(
        eager_fingerprint.fingerprint().to_string(),
        "bf90056c31966ae404d870eac602159d1c90da008212f1c6684a9f10b0b8614a"
    );

    let lazy = Fixture::new(true, Corruption::None);
    let lazy_objects = lazy.objects();
    let lazy_verified = verify_strong_initialization_registrations_v1(
        lazy.verified_patch_sites(),
        lazy.plan.clone(),
        &lazy_objects,
    )
    .unwrap();
    let lazy_fingerprint = compute_strong_initialization_registration_object_fingerprints_v1(
        lazy_verified,
        &lazy_objects,
    )
    .unwrap()
    .fingerprints()[0]
        .fingerprint();

    assert_ne!(eager_fingerprint.fingerprint(), lazy_fingerprint);
}

#[test]
fn initialization_registration_object_hashing_rechecks_object_bytes() {
    let fixture = Fixture::new(false, Corruption::None);
    let original = fixture.objects();
    let verified = verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        &original,
    )
    .unwrap();
    let mut changed = fixture.object_bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let changed = [crate::link_object::ScoopLirObjectCandidateV1::new(
        fixture.member,
        &changed,
    )];

    assert!(matches!(
        compute_strong_initialization_registration_object_fingerprints_v1(verified, &changed),
        Err(StrongInitializationRegistrationObjectFingerprintError::ObjectValidation(
            StrongInitializationRegistrationValidationError::ObjectBytesMismatch(member)
        )) if member == fixture.member
    ));
}

#[test]
fn computes_canonical_initialization_definition_leaves() {
    let fixture = Fixture::new(false, Corruption::None);
    let objects = fixture.objects();
    let registrations = verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        &objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_initialization_registration_object_fingerprints_v1(registrations, &objects)
            .unwrap();
    let definitions =
        compute_strong_initialization_definition_fingerprints_v1(registration_objects, &objects)
            .unwrap();

    assert_eq!(definitions.fingerprints().len(), 1);
    let actual = definitions.fingerprints()[0];
    let plan = &fixture.plan.registrations()[0];
    assert_eq!(actual.unit(), plan.semantic().unit());
    assert_eq!(actual.cell_node(), plan.cell_definition_node());
    assert_eq!(
        actual.cell().to_string(),
        "fef44222d61515cf41819495ce12d74e5d045b1a8aa1aab159bed810e3658735"
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
        &scoop_lir::CanonicalShapeLirDefinitionsV1::new(Vec::new(), &eager.foundation).unwrap(),
    )
    .unwrap();

    let eager_actual = eager_fingerprints.fingerprints()[0];
    let eager_plan = &eager.plan.registrations()[0];
    let eager_gateway = eager_plan.schedule().gateway().unwrap();
    assert_eq!(eager_actual.unit(), eager_plan.semantic().unit());
    assert_eq!(
        eager_actual.registration_node(),
        eager_plan.registration_fingerprint_node()
    );
    assert_eq!(eager_actual.gateway_body(), Some(eager_gateway.body()));
    assert_eq!(
        eager_actual.gateway_definition_node(),
        Some(eager_gateway.body_definition_node())
    );
    assert_eq!(
        eager_actual.registration().to_string(),
        "e226e2bd79982746af91c4e7b63bd6d6d29f8d0177f81f78b0f9969948c5554e"
    );
    let eager_patched =
        crate::link_object::strong_registration_finalization::patch_initializations_for_test(
            &eager_fingerprints,
            &eager_objects,
        )
        .unwrap();
    let eager_verified = &eager_fingerprints
        .definitions()
        .registration_objects()
        .registrations()
        .registrations()[0];
    assert_eq!(
        digest_at(
            &eager_patched[0].1,
            eager_verified
                .registration_definition_patch()
                .checked_offset()
        ),
        eager_actual.registration().as_array()
    );
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
        &scoop_lir::CanonicalShapeLirDefinitionsV1::new(Vec::new(), &lazy.foundation).unwrap(),
    )
    .unwrap();
    let lazy_actual = lazy_fingerprints.fingerprints()[0];
    assert_eq!(lazy_actual.gateway_body(), None);
    assert_eq!(lazy_actual.gateway_definition_node(), None);
    assert_eq!(lazy_actual.gateway_definition(), None);
    assert_ne!(eager_actual.registration(), lazy_actual.registration());
    let lazy_patched =
        crate::link_object::strong_registration_finalization::patch_initializations_for_test(
            &lazy_fingerprints,
            &lazy_objects,
        )
        .unwrap();
    let lazy_verified = &lazy_fingerprints
        .definitions()
        .registration_objects()
        .registrations()
        .registrations()[0];
    assert_eq!(
        digest_at(
            &lazy_patched[0].1,
            lazy_verified
                .registration_definition_patch()
                .checked_offset()
        ),
        lazy_actual.registration().as_array()
    );
    assert!(lazy_verified.gateway_definition_patch().is_none());
}

fn digest_at(bytes: &[u8], offset: u64) -> &[u8] {
    let start = usize::try_from(offset).unwrap();
    &bytes[start..start + 32]
}

fn initialization_definition_fingerprints(
    fixture: &Fixture,
    objects: &[crate::link_object::ScoopLirObjectCandidateV1<'_>],
) -> VerifiedStrongInitializationDefinitionFingerprintSetV1 {
    let registrations = verify_strong_initialization_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.plan.clone(),
        objects,
    )
    .unwrap();
    let registration_objects =
        compute_strong_initialization_registration_object_fingerprints_v1(registrations, objects)
            .unwrap();
    compute_strong_initialization_definition_fingerprints_v1(registration_objects, objects).unwrap()
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
