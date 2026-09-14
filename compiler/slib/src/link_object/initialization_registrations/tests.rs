mod support;

use scoop_lir::{DigestNodeV1, StrongDigestFinalizationPlanV1};

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
    assert_eq!(registration.descriptor_checked_offset() % 8, 0);
    assert_eq!(registration.checked_offset() % 8, 0);
    assert_eq!(
        registration
            .coordinator_diagnostic_relocation()
            .offset_within_atom(),
        40
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
            Corruption::CoordinatorByte,
            InitializationArtifactRoleV1::CoordinatorDescriptor,
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
                role: InitializationRelocationRoleV1::CoordinatorDiagnostic,
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
    let digest_plan = StrongDigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
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
        "e33aaabaca1f05ee46642103c796b4a2b87d4e6cbcad896dee204659c9d76a27"
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
