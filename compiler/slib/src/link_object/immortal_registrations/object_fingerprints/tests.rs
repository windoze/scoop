use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ScoopLirObjectCandidateV1, compute_strong_immortal_object_registration_object_fingerprints_v1,
    verify_strong_immortal_object_registrations_v1,
};

#[test]
fn hashes_the_exact_string_object_and_descriptor_relocation() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let requirements = fixture.undefined_requirements();
    let registration_objects = registration_objects(&fixture, &objects);

    let fingerprints = compute_strong_immortal_object_definition_fingerprints_v1(
        registration_objects,
        requirements,
        &objects,
    )
    .unwrap();

    assert_eq!(fingerprints.fingerprints().len(), 1);
    let actual = fingerprints.fingerprints()[0];
    let plan = fixture.immortal_registration_plan.registrations()[0];
    assert_eq!(actual.object(), plan.object());
    assert_eq!(actual.node(), plan.object_definition_node());
    assert_eq!(
        actual.fingerprint().to_string(),
        "80264eaa8346aca094d5c905650e480421117ef9cece159fcb08a628d55fce29"
    );
}

#[test]
fn rejects_a_string_extent_inconsistent_with_its_length() {
    let fixture = Fixture::new(Corruption::ImmortalObjectLength);
    let object = fixture.immortal_registration_plan.registrations()[0].object();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let requirements = fixture.undefined_requirements();
    let registration_objects = registration_objects(&fixture, &objects);

    assert_eq!(
        compute_strong_immortal_object_definition_fingerprints_v1(
            registration_objects,
            requirements,
            &objects,
        ),
        Err(
            StrongImmortalObjectDefinitionFingerprintError::ObjectBytes {
                object,
                kind: ImmortalObjectByteFailureV1::LengthOverflow,
            }
        )
    );
}

#[test]
fn rejects_a_descriptor_relocation_to_an_unrelated_strong_definition() {
    let fixture = Fixture::new(Corruption::ImmortalObjectDescriptorRelocationTarget);
    let object = fixture.immortal_registration_plan.registrations()[0].object();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let requirements = fixture.undefined_requirements();
    let registration_objects = registration_objects(&fixture, &objects);

    assert_eq!(
        compute_strong_immortal_object_definition_fingerprints_v1(
            registration_objects,
            requirements,
            &objects,
        ),
        Err(
            StrongImmortalObjectDefinitionFingerprintError::DescriptorRelocation {
                object,
                kind: ImmortalObjectRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rechecks_object_bytes_after_all_input_proofs() {
    let mut fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];
    let requirements = fixture.undefined_requirements();
    let registration_objects = registration_objects(&fixture, &objects);
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let changed = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        compute_strong_immortal_object_definition_fingerprints_v1(
            registration_objects,
            requirements,
            &changed,
        ),
        Err(
            StrongImmortalObjectDefinitionFingerprintError::ObjectValidation(
                StrongImmortalObjectRegistrationValidationError::ObjectBytesMismatch(
                    fixture.member
                )
            )
        )
    );
}

fn registration_objects<'a>(
    fixture: &Fixture,
    objects: &'a [ScoopLirObjectCandidateV1<'a>],
) -> VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
    let registrations = verify_strong_immortal_object_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.immortal_registration_plan.clone(),
        objects,
    )
    .unwrap();
    compute_strong_immortal_object_registration_object_fingerprints_v1(registrations, objects)
        .unwrap()
}
