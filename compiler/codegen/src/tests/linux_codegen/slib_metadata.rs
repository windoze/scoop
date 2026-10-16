use super::slib_support::SlibObjects;
use scoop_slib::*;

#[test]
fn elf_static_immortal_and_lazy_initialization_registrations() {
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
        let sites = fixture.sites(builtins.clone());
        let production = fixture.emitted.production().registration_production();
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
        let initializations =
            bind_initialization_registrations_v1(&sites, production.initialization_units().clone())
                .unwrap();
        assert_eq!(initializations.registrations().len(), 2);
        assert!(
            initializations
                .registrations()
                .iter()
                .all(|registration| registration.gateway_definition_patch().is_none())
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
    let sites = fixture.sites(fixture.builtins(&fixture.objects));
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
