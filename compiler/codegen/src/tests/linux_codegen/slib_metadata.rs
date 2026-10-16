use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;
use scoop_slib::*;

#[test]
fn elf_static_immortal_and_initialization_registrations() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let fixture = SlibObjects::new(
            super::slib_metadata_fixture::module(target),
            directory.path(),
        );
        let builtins = fixture.builtins(&fixture.objects);
        let sites = fixture.sites(builtins.clone(), &fixture.objects);
        let production = fixture.emitted.production().registration_production();
        let objects = candidates(&fixture.objects);
        let immortals = compute_strong_immortal_object_fingerprints_v1(
            production.immortal_objects().clone(),
            fixture.emitted.production().canonical_shape_definitions(),
        )
        .expect("ELF immortal String registration ABIs");
        assert_eq!(immortals.fingerprints().len(), 2);
        let storages =
            bind_static_storage_registrations_v1(&sites, production.static_storages().clone())
                .expect("ELF static storage digest slots");
        assert_eq!(storages.registrations().len(), 7);
        assert_storage_slots(&storages, &fixture.objects);
        let initializations = verify_strong_initialization_registrations_v1(
            sites,
            production.initialization_units().clone(),
            &objects,
        )
        .expect("ELF initialization diagnostic and callable pointers");
        assert_eq!(initializations.registrations().len(), 2);
        let diagnostic = initializations.registrations()[0].registration_diagnostic_relocation();
        let mut damaged = fixture.objects.clone();
        set_rela_addend(
            &mut damaged,
            &builtins,
            diagnostic.member(),
            diagnostic.containing_atom(),
            diagnostic.offset_within_atom(),
            -1,
        );
        let damaged_sites = fixture.sites(fixture.builtins(&damaged), &damaged);
        assert!(
            verify_strong_initialization_registrations_v1(
                damaged_sites,
                production.initialization_units().clone(),
                &candidates(&damaged),
            )
            .is_err()
        );
    }
}

#[test]
fn elf_encoded_storage_digest_slots() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for initial in [0, 42] {
            let fixture = SlibObjects::new(
                super::slib_metadata_fixture::encoded_only(target, initial),
                directory.path(),
            );
            check_storage_slots(&fixture);
        }
    }
}

#[test]
fn elf_partitioned_zeroed_storage_digest_slots() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let fixture = SlibObjects::new(
            super::slib_metadata_fixture::zeroed_odr_only(target),
            directory.path(),
        );
        check_storage_slots(&fixture);
    }
}

fn check_storage_slots(fixture: &SlibObjects) {
    let sites = fixture.sites(fixture.builtins(&fixture.objects), &fixture.objects);
    let data = bind_static_storage_registrations_v1(
        &sites,
        fixture
            .emitted
            .production()
            .registration_production()
            .static_storages()
            .clone(),
    )
    .unwrap();
    assert_eq!(data.registrations().len(), 1);
    assert_storage_slots(&data, &fixture.objects);
}

fn assert_storage_slots(
    data: &StaticStorageRegistrationDataV1,
    objects: &[(SlibMemberId, Vec<u8>)],
) {
    for registration in data.registrations() {
        for patch in [
            registration.scan_fingerprint_patch(),
            registration.layout_fingerprint_patch(),
        ] {
            let bytes = &objects
                .iter()
                .find(|(member, _)| *member == patch.member())
                .unwrap()
                .1;
            let offset = usize::try_from(patch.checked_offset()).unwrap();
            assert_eq!(&bytes[offset..offset + 32], &[0; 32]);
        }
    }
}
