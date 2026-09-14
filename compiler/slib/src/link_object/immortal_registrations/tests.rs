use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1, verify_scoop_lir_digest_patch_sites_v1,
};
use scoop_lir::{DigestNodeV1, StrongDigestFinalizationPlanV1};

#[test]
fn verifies_record_object_type_relocations_and_patch_site() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_immortal_object_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.immortal_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    assert_eq!(
        verified.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(verified.registrations().len(), 1);
    let registration = &verified.registrations()[0];
    let plan = fixture.immortal_registration_plan.registrations()[0];
    assert_eq!(registration.object(), plan.object());
    assert_eq!(registration.member(), fixture.member);
    assert_eq!(registration.object_relocation().offset_within_atom(), 152);
    assert_eq!(
        registration
            .type_registration_relocation()
            .offset_within_atom(),
        176
    );
    assert_eq!(registration.object_relocation().width_bytes(), 8);
    assert_eq!(
        registration
            .registration_definition_patch()
            .checked_offset(),
        registration.checked_offset() + 120
    );
}

#[test]
fn rejects_noncanonical_immortal_registration_bytes() {
    let fixture = Fixture::new(Corruption::ImmortalRegistrationMagic);
    let object = fixture.immortal_registration_plan.registrations()[0].object();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_strong_immortal_object_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::RecordByteMismatch {
                object: actual,
                offset_within_atom: 0,
                ..
            }
        ) if actual == object
    ));
}

#[test]
fn rejects_object_relocation_to_the_registration_itself() {
    let fixture = Fixture::new(Corruption::ImmortalObjectRelocationTarget);
    let object = fixture.immortal_registration_plan.registrations()[0].object();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::ObjectRelocationMismatch {
                object,
                kind: ImmortalObjectRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rejects_type_relocation_to_the_immortal_object() {
    let fixture = Fixture::new(Corruption::ImmortalTypeRegistrationRelocationTarget);
    let object = fixture.immortal_registration_plan.registrations()[0].object();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::TypeRegistrationRelocationMismatch {
                object,
                kind: ImmortalObjectRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn accepts_the_exact_core_external_type_registration() {
    let fixture = Fixture::new(Corruption::CoreExternalImmortalTypeRegistration);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_immortal_object_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.immortal_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    assert!(matches!(
        verified.registrations()[0]
            .type_registration_relocation()
            .resolution(),
        crate::link_object::StrongRelocationResolutionV1::ExternalCandidate { .. }
    ));
}

#[test]
fn rejects_a_registration_node_with_the_wrong_direct_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.immortal_registration_plan.registrations()[0];
    let nodes = fixture
        .digest_plan
        .nodes()
        .iter()
        .map(|node| {
            if node.id() != plan.registration_fingerprint_node() {
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
    let wrong_digest_plan =
        StrongDigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        wrong_digest_plan,
        &objects,
        &fixture.provisional_patch_sites,
    )
    .unwrap();

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            patch_sites,
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                object: plan.object(),
                kind: ImmortalObjectRegistrationDigestPlanFailureV1::RegistrationDirectInputs,
            }
        )
    );
}

#[test]
fn rejects_a_non_leaf_immortal_object_definition() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.immortal_registration_plan.registrations()[0];
    let lir_definition = DigestNodeV1::new(
        scoop_identity::DigestNodeKey::lir_definition(plan.object_primary_atom()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let nodes = std::iter::once(lir_definition.clone())
        .chain(fixture.digest_plan.nodes().iter().map(|node| {
            if node.id() != plan.object_definition_node() {
                return node.clone();
            }
            DigestNodeV1::new(
                *node.key(),
                vec![scoop_lir::DigestInputRefV1::from_node(&lir_definition)],
                Vec::new(),
            )
            .unwrap()
        }))
        .collect();
    let wrong_digest_plan =
        StrongDigestFinalizationPlanV1::new(nodes, &fixture.foundation).unwrap();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        wrong_digest_plan,
        &objects,
        &fixture.provisional_patch_sites,
    )
    .unwrap();

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            patch_sites,
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::DigestPlanMismatch {
                object: plan.object(),
                kind: ImmortalObjectRegistrationDigestPlanFailureV1::
                    ImmortalObjectDefinitionDirectInputs,
            }
        )
    );
}

#[test]
fn rejects_a_digest_slot_materialized_at_the_wrong_field() {
    let fixture = Fixture::new(Corruption::None);
    let plan = fixture.immortal_registration_plan.registrations()[0];
    let provisional = fixture
        .provisional_patch_sites
        .iter()
        .map(|site| {
            let offset = if site.intent() == plan.registration_definition_patch() {
                site.checked_offset() - 8
            } else {
                site.checked_offset()
            };
            ProvisionalDigestPatchSiteV1::new(
                site.intent(),
                site.member(),
                offset,
                site.width_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins.clone(),
        &fixture.foundation,
        fixture.digest_plan.clone(),
        &objects,
        &provisional,
    )
    .unwrap();

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            patch_sites,
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongImmortalObjectRegistrationValidationError::PatchMismatch {
                object: plan.object(),
                intent: plan.registration_definition_patch(),
                kind: ImmortalObjectRegistrationPatchFailureV1::OffsetWithinAtom,
            }
        )
    );
}

#[test]
fn rejects_object_bytes_changed_after_all_input_proofs() {
    let mut fixture = Fixture::new(Corruption::None);
    let patch_sites = fixture.verified_patch_sites();
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_immortal_object_registrations_v1(
            patch_sites,
            fixture.immortal_registration_plan.clone(),
            &objects,
        ),
        Err(StrongImmortalObjectRegistrationValidationError::ObjectBytesMismatch(fixture.member))
    );
}
