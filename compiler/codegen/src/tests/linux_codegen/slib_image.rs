use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;
use scoop_slib::*;

#[test]
fn elf_cone_image_references_its_support_atoms_and_registration_tables() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        for module in [
            for_target(
                moving_gc::qualification::stackmap_qualification_module(),
                target,
            ),
            super::slib_metadata_fixture::module(target),
        ] {
            let fixture = SlibObjects::new(module, directory.path());
            let builtins = fixture.builtins(&fixture.objects);
            let verify = |objects: &[(SlibMemberId, Vec<u8>)]| {
                verify_cone_image_v1(
                    fixture.sites(fixture.builtins(objects), objects),
                    fixture.emitted.production().image_plan().clone(),
                    &candidates(objects),
                )
            };
            let image = verify(&fixture.objects).expect("real ELF Cone image");
            assert_eq!(image.support_relocations().len(), 10);
            assert!(!image.registration_relocations().is_empty());
            let pointer = &image.support_relocations()[0];
            let mut damaged = fixture.objects.clone();
            set_rela_addend(
                &mut damaged,
                &builtins,
                pointer.member(),
                pointer.containing_atom(),
                pointer.offset_within_atom(),
                -1,
            );
            assert!(verify(&damaged).is_err(), "invalid local support pointer");
            let entry = &image.registration_relocations()[0];
            let mut damaged = fixture.objects.clone();
            set_rela_addend(
                &mut damaged,
                &builtins,
                entry.source_member(),
                entry.containing_atom(),
                entry.offset_within_atom(),
                8,
            );
            assert!(
                verify(&damaged).is_err(),
                "registration pointer must refer to its start"
            );
        }
    }
}
