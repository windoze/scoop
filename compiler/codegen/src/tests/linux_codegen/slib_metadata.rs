use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;
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
        compute_strong_immortal_object_registration_object_fingerprints_v1(immortals, &objects)
            .expect("immortal registration canonical pointers");
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
        compute_strong_static_storage_registration_object_fingerprints_v1(storages, &objects)
            .expect("static registration canonical pointers");
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
