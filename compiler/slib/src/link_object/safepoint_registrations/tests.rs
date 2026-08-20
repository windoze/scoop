use super::*;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{
    ProvisionalDigestPatchSiteV1, ScoopLirObjectCandidateV1, verify_scoop_lir_digest_patch_sites_v1,
};

#[test]
fn verifies_exact_registration_records_against_stackmaps_and_patch_sites() {
    let fixture = Fixture::new(Corruption::None);
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_safepoint_registrations_v1(
        stackmaps,
        patch_sites,
        fixture.registration_plan.clone(),
        &objects,
    )
    .unwrap();

    assert_eq!(
        verified.producer(),
        scoop_identity::ConeIdentity::SINGLE_FILE
    );
    assert_eq!(verified.registrations().len(), 2);
    assert!(
        verified
            .registrations()
            .windows(2)
            .all(|pair| pair[0].site() < pair[1].site())
    );
    for registration in verified.registrations() {
        assert_eq!(registration.member(), fixture.member);
        assert_eq!(
            registration.normalized_stackmap_patch().checked_offset(),
            registration.checked_offset() + 168
        );
        assert_eq!(registration.normalized_stackmap_patch().width_bytes(), 32);
    }
}

#[test]
fn rejects_a_registration_with_noncanonical_record_bytes() {
    let fixture = Fixture::new(Corruption::RegistrationMagic);
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let first_site = fixture.registration_plan.registrations()[0].site();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_strong_safepoint_registrations_v1(
            stackmaps,
            patch_sites,
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(StrongSafepointRegistrationValidationError::RecordByteMismatch {
            site,
            offset_within_atom: 0,
            ..
        }) if site == first_site
    ));
}

#[test]
fn rejects_registration_records_in_writable_storage() {
    let fixture = Fixture::new(Corruption::WritableRegistrationSection);
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let first_site = fixture.registration_plan.registrations()[0].site();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_safepoint_registrations_v1(
            stackmaps,
            patch_sites,
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongSafepointRegistrationValidationError::PrimaryAtomSectionMismatch {
                site: first_site,
            }
        )
    );
}

#[test]
fn rejects_relocations_inside_a_registration_record() {
    let fixture = Fixture::new(Corruption::RelocatedRegistration);
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let first_site = fixture.registration_plan.registrations()[0].site();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_safepoint_registrations_v1(
            stackmaps,
            patch_sites,
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongSafepointRegistrationValidationError::UnexpectedRelocation {
                site: first_site,
                offset_within_atom: 56,
            }
        )
    );
}

#[test]
fn rejects_object_bytes_changed_after_all_input_proofs() {
    let mut fixture = Fixture::new(Corruption::None);
    let stackmaps = fixture.verified_stackmaps();
    let patch_sites = fixture.verified_patch_sites();
    let last = fixture.object_bytes.len() - 1;
    fixture.object_bytes[last] ^= 1;
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_safepoint_registrations_v1(
            stackmaps,
            patch_sites,
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(StrongSafepointRegistrationValidationError::ObjectBytesMismatch(fixture.member,))
    );
}

#[test]
fn rejects_stackmap_and_patch_proofs_from_different_object_sets() {
    let fixture = Fixture::new(Corruption::None);
    let other = Fixture::new(Corruption::RegistrationMagic);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_safepoint_registrations_v1(
            fixture.verified_stackmaps(),
            other.verified_patch_sites(),
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(StrongSafepointRegistrationValidationError::ObjectProofMismatch)
    );
}

#[test]
fn rejects_a_digest_slot_materialized_at_the_wrong_record_field() {
    let fixture = Fixture::new(Corruption::None);
    let first = fixture.registration_plan.registrations()[0];
    let provisional = fixture
        .provisional_patch_sites
        .iter()
        .map(|site| {
            let offset = if site.intent() == first.normalized_stackmap_patch() {
                site.checked_offset() - 112
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
        verify_strong_safepoint_registrations_v1(
            fixture.verified_stackmaps(),
            patch_sites,
            fixture.registration_plan.clone(),
            &objects,
        ),
        Err(StrongSafepointRegistrationValidationError::PatchMismatch {
            site: first.site(),
            intent: first.normalized_stackmap_patch(),
            kind: SafepointRegistrationPatchFailureV1::OffsetWithinAtom,
        })
    );
}
