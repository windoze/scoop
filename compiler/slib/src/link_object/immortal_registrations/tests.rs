use super::*;
use crate::link_object::ScoopLirObjectCandidateV1;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};

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
    assert_eq!(registration.object_relocation().offset_within_atom(), 120);
    assert_eq!(
        registration
            .type_registration_relocation()
            .offset_within_atom(),
        144
    );
    assert_eq!(registration.object_relocation().width_bytes(), 8);
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

#[test]
fn rejects_invalid_immortal_string_payload_and_descriptor() {
    for (corruption, kind) in [
        (
            Corruption::ImmortalObjectLength,
            ImmortalObjectBodyFailureV1::Length,
        ),
        (
            Corruption::ImmortalObjectDescriptorRelocationTarget,
            ImmortalObjectBodyFailureV1::DescriptorRelocation,
        ),
    ] {
        let fixture = Fixture::new(corruption);
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
                StrongImmortalObjectRegistrationValidationError::InvalidObjectBody {
                    object: fixture.immortal_registration_plan.registrations()[0].object(),
                    kind,
                }
            )
        );
    }
}
