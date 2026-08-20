use super::*;
use crate::link_object::ScoopLirObjectCandidateV1;
use crate::link_object::stackmap_normalization::verification::tests::support::{
    Corruption, Fixture,
};
use crate::link_object::{ProvisionalDigestPatchSiteV1, verify_scoop_lir_digest_patch_sites_v1};

#[test]
fn verifies_static_storage_registration_and_initial_artifacts() {
    let fixture = Fixture::new(Corruption::None);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    assert_eq!(verified.registrations().len(), 1);
    let registration = &verified.registrations()[0];
    assert_eq!(registration.storage_relocation().offset_within_atom(), 128);
    assert_eq!(registration.scan_relocation().offset_within_atom(), 160);
    assert_eq!(registration.initial_storage_relocations().len(), 1);
    assert_eq!(registration.initial_target_relocations().len(), 1);
    assert_eq!(
        registration.scan_fingerprint_patch().checked_offset(),
        registration.checked_offset() + 168
    );
    assert_eq!(
        registration.layout_fingerprint_patch().checked_offset(),
        registration.checked_offset() + 200
    );
}

#[test]
fn verifies_zeroed_runtime_storage_with_shared_sentinels() {
    let fixture = Fixture::new(Corruption::StaticZeroedInitialState);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    let registration = &verified.registrations()[0];
    assert!(registration.initial_storage_relocations().is_empty());
    assert!(registration.initial_target_relocations().is_empty());
    assert!(matches!(
        registration.template_relocation().shape(),
        crate::link_object::VerifiedObjectRelocationShapeV1::Unsigned64 {
            target: crate::link_object::VerifiedRelocationTargetV1::LocalDefinition {
                owner_atom: None,
                ..
            }
        }
    ));
}

#[test]
fn rejects_zeroed_runtime_storage_in_file_backed_writable_data() {
    assert_storage_section_mismatch(Corruption::StaticZeroedWritableSection);
}

#[test]
fn rejects_encoded_storage_in_zero_fill() {
    assert_storage_section_mismatch(Corruption::StaticEncodedZeroFillSection);
}

fn assert_storage_section_mismatch(corruption: Corruption) {
    let fixture = Fixture::new(corruption);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan,
            &objects,
        ),
        Err(
            StrongStaticStorageRegistrationValidationError::AtomSectionMismatch {
                storage,
                role: StaticStorageArtifactRoleV1::Storage,
            }
        )
    );
}

#[test]
fn verifies_encoded_null_storage_with_an_empty_relocation_sentinel() {
    let fixture = Fixture::new(Corruption::StaticEncodedEmptyInitialState);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    let verified = verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .unwrap();

    let registration = &verified.registrations()[0];
    assert!(registration.initial_storage_relocations().is_empty());
    assert!(registration.initial_target_relocations().is_empty());
    assert!(matches!(
        registration.template_relocation().shape(),
        crate::link_object::VerifiedObjectRelocationShapeV1::Unsigned64 {
            target: crate::link_object::VerifiedRelocationTargetV1::LocalDefinition {
                owner_atom: Some(_),
                ..
            }
        }
    ));
    assert!(matches!(
        registration.relocation_table_relocation().shape(),
        crate::link_object::VerifiedObjectRelocationShapeV1::Unsigned64 {
            target: crate::link_object::VerifiedRelocationTargetV1::LocalDefinition {
                owner_atom: None,
                ..
            }
        }
    ));
}

#[test]
fn accepts_shared_zero_bytes_for_both_typed_empty_spans() {
    let fixture = Fixture::new(Corruption::StaticAliasedSentinels);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    verify_strong_static_storage_registrations_v1(
        fixture.verified_patch_sites(),
        fixture.static_storage_registration_plan.clone(),
        &objects,
    )
    .expect("empty spans have no identity beyond their typed readable range");
}

#[test]
fn rejects_noncanonical_static_storage_registration_bytes() {
    let fixture = Fixture::new(Corruption::StaticRegistrationMagic);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(StrongStaticStorageRegistrationValidationError::RecordByteMismatch {
            storage: actual,
            offset_within_atom: 0,
            ..
        }) if actual == storage
    ));
}

#[test]
fn rejects_noncanonical_scan_program_bytes() {
    let fixture = Fixture::new(Corruption::StaticScanProgram);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert!(matches!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(StrongStaticStorageRegistrationValidationError::ArtifactByteMismatch {
            storage: actual,
            role: StaticStorageArtifactRoleV1::ScanProgram,
            offset_within_atom: 0,
            ..
        }) if actual == storage
    ));
}

#[test]
fn rejects_storage_pointer_to_the_registration() {
    let fixture = Fixture::new(Corruption::StaticStorageRelocationTarget);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongStaticStorageRegistrationValidationError::RelocationMismatch {
                storage,
                role: StaticStorageRelocationRoleV1::StoragePointer,
                kind: StaticStorageRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rejects_initial_storage_pointer_to_an_immortal_registration() {
    let fixture = Fixture::new(Corruption::StaticInitialStorageRelocationTarget);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongStaticStorageRegistrationValidationError::RelocationMismatch {
                storage,
                role: StaticStorageRelocationRoleV1::InitialStorageValue { index: 0 },
                kind: StaticStorageRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rejects_initial_table_pointer_to_an_immortal_object() {
    let fixture = Fixture::new(Corruption::StaticInitialTableRelocationTarget);
    let storage = fixture.static_storage_registration_plan.registrations()[0]
        .semantic()
        .storage();
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.object_bytes,
    )];

    assert_eq!(
        verify_strong_static_storage_registrations_v1(
            fixture.verified_patch_sites(),
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongStaticStorageRegistrationValidationError::RelocationMismatch {
                storage,
                role: StaticStorageRelocationRoleV1::InitialRelocationTarget { index: 0 },
                kind: StaticStorageRegistrationRelocationFailureV1::TargetDefinition,
            }
        )
    );
}

#[test]
fn rejects_a_digest_slot_materialized_at_the_wrong_field() {
    let fixture = Fixture::new(Corruption::None);
    let plan = &fixture.static_storage_registration_plan.registrations()[0];
    let provisional = fixture
        .provisional_patch_sites
        .iter()
        .map(|site| {
            let offset = if site.intent() == plan.scan_fingerprint_patch() {
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
        verify_strong_static_storage_registrations_v1(
            patch_sites,
            fixture.static_storage_registration_plan.clone(),
            &objects,
        ),
        Err(
            StrongStaticStorageRegistrationValidationError::PatchMismatch {
                storage: plan.semantic().storage(),
                intent: plan.scan_fingerprint_patch(),
                kind: StaticStorageRegistrationPatchFailureV1::OffsetWithinAtom,
            }
        )
    );
}
