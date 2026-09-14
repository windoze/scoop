use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1, verify_scoop_lir_digest_patch_sites_v1,
};
use scoop_lir::{DigestNodeV1, StrongDigestFinalizationPlanV1};

#[test]
fn verifies_exact_type_record_descriptor_relocation_and_three_patch_sites() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_type_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.type_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    assert_eq!(
        verified.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(verified.registrations().len(), 1);
    let registration = &verified.registrations()[0];
    let plan = &fixture.type_registration_plan.registrations()[0];
    assert_eq!(registration.exact_type(), plan.exact_type());
    assert_eq!(registration.member(), fixture.member);
    assert_eq!(
        registration.descriptor_relocation().offset_within_atom(),
        168
    );
    assert_eq!(registration.descriptor_relocation().width_bytes(), 8);
    assert_eq!(
        registration
            .registration_definition_patch()
            .checked_offset(),
        registration.checked_offset() + 120
    );
    assert_eq!(
        registration.descriptor_definition_patch().checked_offset(),
        registration.checked_offset() + 176
    );
    assert_eq!(
        registration.layout_fingerprint_patch().checked_offset(),
        registration.checked_offset() + 208
    );
}

#[test]
fn rejects_noncanonical_type_registration_bytes() {
    let fixture = Fixture::new(Corruption::TypeRegistrationMagic);
    let exact_type = fixture.type_registration_plan.registrations()[0].exact_type();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_strong_type_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.type_registration_plan.clone(),
            &objects,
        ),
        Err(StrongTypeRegistrationValidationError::RecordByteMismatch {
            exact_type: actual,
            offset_within_atom: 0,
            ..
        }) if actual == exact_type
    ));
}

#[test]
fn rejects_descriptor_relocation_to_the_registration_itself() {
    let fixture = Fixture::new(Corruption::TypeDescriptorRelocationTarget);
    let exact_type = fixture.type_registration_plan.registrations()[0].exact_type();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_type_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.type_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongTypeRegistrationValidationError::DescriptorRelocationMismatch {
                exact_type,
                kind: TypeRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rejects_a_registration_node_with_the_wrong_direct_inputs() {
    let fixture = Fixture::new(Corruption::None);
    let plan = &fixture.type_registration_plan.registrations()[0];
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
        verify_strong_type_registrations_v1(
            patch_sites,
            fixture.type_registration_plan.clone(),
            &objects,
        ),
        Err(StrongTypeRegistrationValidationError::DigestPlanMismatch {
            exact_type: plan.exact_type(),
            kind: TypeRegistrationDigestPlanFailureV1::RegistrationDirectInputs,
        })
    );
}

#[test]
fn rejects_a_digest_slot_materialized_at_the_wrong_field() {
    let fixture = Fixture::new(Corruption::None);
    let plan = &fixture.type_registration_plan.registrations()[0];
    let provisional = fixture
        .provisional_patch_sites
        .iter()
        .map(|site| {
            let offset = if site.intent() == plan.layout_fingerprint_patch() {
                site.checked_offset() - 128
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
        verify_strong_type_registrations_v1(
            patch_sites,
            fixture.type_registration_plan.clone(),
            &objects,
        ),
        Err(StrongTypeRegistrationValidationError::PatchMismatch {
            exact_type: plan.exact_type(),
            intent: plan.layout_fingerprint_patch(),
            kind: TypeRegistrationPatchFailureV1::OffsetWithinAtom,
        })
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
        verify_strong_type_registrations_v1(
            patch_sites,
            fixture.type_registration_plan.clone(),
            &objects,
        ),
        Err(StrongTypeRegistrationValidationError::ObjectBytesMismatch(
            fixture.member,
        ))
    );
}
