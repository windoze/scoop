use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;
use object::ObjectSymbol;
use scoop_slib::*;

#[test]
fn elf_static_immortal_and_initialization_registrations() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let fixture = SlibObjects::new(
            super::slib_metadata_fixture::module(target),
            directory.path(),
        );
        let builtins = fixture.builtins(&fixture.objects);
        let sites = fixture.sites(builtins.clone(), &fixture.objects);
        let production = fixture.emitted.production().registration_production();
        let objects = candidates(&fixture.objects);
        let immortals = verify_strong_immortal_object_registrations_v1(
            sites.clone(),
            production.immortal_objects().clone(),
            &objects,
        )
        .expect("ELF immortal String registrations");
        assert_eq!(immortals.registrations().len(), 2);
        immortals;
        let storages = verify_strong_static_storage_registrations_v1(
            sites.clone(),
            production.static_storages().clone(),
            &objects,
        )
        .expect("ELF initial templates, sentinels and immortal relocations");
        assert_eq!(storages.registrations().len(), 7);
        let template = storages
            .registrations()
            .iter()
            .find(|registration| {
                registration
                    .template_relocation()
                    .shape()
                    .form()
                    .absolute64_addend(0)
                    != Some(0)
            })
            .unwrap()
            .template_relocation();
        let mut damaged = fixture.objects.clone();
        set_rela_addend(
            &mut damaged,
            &builtins,
            template.member(),
            template.containing_atom(),
            template.offset_within_atom(),
            -1,
        );
        let damaged_sites = fixture.sites(fixture.builtins(&damaged), &damaged);
        assert!(
            verify_strong_static_storage_registrations_v1(
                damaged_sites,
                production.static_storages().clone(),
                &candidates(&damaged),
            )
            .is_err()
        );
        storages;
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
fn elf_encoded_storage_accepts_only_zero_sentinel_prefixes() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for initial in [0, 42] {
            let fixture = SlibObjects::new(
                super::slib_metadata_fixture::encoded_only(target, initial),
                directory.path(),
            );
            let sites = fixture.sites(fixture.builtins(&fixture.objects), &fixture.objects);
            let storages = verify_strong_static_storage_registrations_v1(
                sites,
                fixture
                    .emitted
                    .production()
                    .registration_production()
                    .static_storages()
                    .clone(),
                &candidates(&fixture.objects),
            )
            .expect("a static scalar needs no immortal or initialization unit");
            assert_eq!(storages.registrations().len(), 1);
            storages;
            let mut damaged = fixture.objects.clone();
            let (bytes, offset) = damaged
                .iter_mut()
                .find_map(|(_, bytes)| {
                    let file = object::File::parse(bytes.as_slice()).unwrap();
                    let offset = file.sections().find_map(|section| {
                        if section.name() != Ok(".data.rel.ro.scoop.metadata") {
                            return None;
                        }
                        let first_atom = file
                            .symbols()
                            .filter(|symbol| {
                                symbol.section_index() == Some(section.index())
                                    && symbol
                                        .name()
                                        .is_ok_and(|name| name.starts_with("scoop$1$bs$"))
                            })
                            .map(|symbol| symbol.address())
                            .min()?;
                        (first_atom > 0).then(|| section.file_range().unwrap().0 as usize)
                    })?;
                    Some((bytes, offset))
                })
                .expect("LLVM emits a shared empty sentinel before named metadata atoms");
            bytes[offset] = 1;
            assert!(matches!(
                fixture.try_builtins(&damaged),
                Err(BuiltinObjectSetValidationError::StrongDefinitions {
                    source: StrongObjectDefinitionValidationError::UnownedSectionPrefix { .. },
                    ..
                })
            ));
        }
    }
}
