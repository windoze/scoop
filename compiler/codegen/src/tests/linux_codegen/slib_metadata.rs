use super::slib_support::SlibObjects;
use object::{Object, ObjectSection, ObjectSymbol};
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
        let production = fixture.emitted.production().registration_production();
        let immortals = compute_strong_immortal_object_fingerprints_v1(
            production.immortal_objects().clone(),
            fixture.emitted.production().canonical_shape_definitions(),
        )
        .expect("ELF immortal String registration ABIs");
        assert_eq!(immortals.fingerprints().len(), 2);
        let storages = production.static_storages();
        assert_eq!(storages.registrations().len(), 7);
        assert_storage_records(storages, &builtins, &fixture.objects);
        let initializations = production.initialization_units().clone();
        assert_eq!(initializations.registrations().len(), 2);
        assert!(
            initializations
                .registrations()
                .iter()
                .all(|registration| registration.schedule().gateway().is_none())
        );
    }
}

#[test]
fn elf_encoded_storage_records() {
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
            check_storage_records(&fixture);
        }
    }
}

#[test]
fn elf_partitioned_zeroed_storage_records() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let fixture = SlibObjects::new(
            super::slib_metadata_fixture::zeroed_odr_only(target),
            directory.path(),
        );
        check_storage_records(&fixture);
    }
}

fn check_storage_records(fixture: &SlibObjects) {
    let builtins = fixture.builtins(&fixture.objects);
    let data = fixture
        .emitted
        .production()
        .registration_production()
        .static_storages()
        .clone();
    assert_eq!(data.registrations().len(), 1);
    assert_storage_records(&data, &builtins, &fixture.objects);
}

fn assert_storage_records(
    data: &StaticStorageRegistrationDataV1,
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    objects: &[(SlibMemberId, Vec<u8>)],
) {
    for registration in data.registrations() {
        let member = builtins
            .member_plan()
            .member_for_definition(registration.registration_definition_plan())
            .unwrap();
        let bytes = &objects.iter().find(|(id, _)| *id == member).unwrap().1;
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let symbol = file
            .symbol_by_name(registration.registration_symbol().symbol().as_str())
            .unwrap();
        let section = file
            .section_by_index(symbol.section_index().unwrap())
            .unwrap();
        let offset = usize::try_from(symbol.address() - section.address()).unwrap();
        let record = &section.data().unwrap()[offset..offset + 200];
        assert_eq!(
            u64::from_le_bytes(record[0..8].try_into().unwrap()),
            0x5343_4f4f_5053_544f
        );
        assert_eq!(
            u32::from_le_bytes(record[8..12].try_into().unwrap()),
            scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1
        );
        assert_eq!(u32::from_le_bytes(record[12..16].try_into().unwrap()), 200);
    }
}
