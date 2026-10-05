//! Type metadata produced by LLVM, consumed through the public slib reader.

use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates, set_rela_addend};
use super::*;

#[test]
fn elf_type_registrations_resolve_descriptor_diagnostic_and_itable_pointers() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let module =
            super::super::objects::classes_module_for(scoop_lir::LirTargetProfile::from_id(target));
        let fixture = SlibObjects::new(module, directory.path());
        let builtins = fixture.builtins(&fixture.objects);
        let verify = |objects: &[(SlibMemberId, Vec<u8>)]| {
            let sites = fixture.sites(fixture.builtins(objects), objects);
            verify_strong_type_registrations_v1(
                sites,
                fixture
                    .emitted
                    .production()
                    .registration_production()
                    .types()
                    .clone(),
                &candidates(objects),
            )
        };
        let registrations = verify(&fixture.objects).expect("real ELF type registrations");
        assert_eq!(registrations.registrations().len(), 4);
        let with_itable = registrations
            .registrations()
            .iter()
            .find(|registration| {
                matches!(
                    registration.descriptor().itable_directory(),
                    VerifiedTypeDescriptorITableDirectoryV1::Defined { .. }
                )
            })
            .unwrap();
        for pointer in [
            with_itable.descriptor().diagnostic_relocation(),
            with_itable
                .descriptor()
                .itable_directory()
                .descriptor_relocation()
                .unwrap(),
        ] {
            let mut damaged = fixture.objects.clone();
            let VerifiedObjectRelocationFormV1::ElfRela { addend, .. } = pointer.shape().form()
            else {
                panic!("ELF metadata must keep RELA facts");
            };
            set_rela_addend(
                &mut damaged,
                &builtins,
                pointer.member(),
                pointer.containing_atom(),
                pointer.offset_within_atom(),
                addend + 1,
            );
            assert!(
                verify(&damaged).is_err(),
                "an in-section pointer to the wrong byte must fail"
            );
        }
    }
}
