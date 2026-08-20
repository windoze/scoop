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
        for odr in [false, true] {
            let mut module = super::super::objects::classes_module_for(
                scoop_lir::LirTargetProfile::from_id(target),
            );
            if odr {
                odr_point(&mut module);
            }
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
            let requirements = fixture.requirements(registrations.patch_sites().clone());
            assert_eq!(requirements.selection().target(), fixture.emitted.target());
            let fingerprints = compute_strong_type_fingerprints_v1(
                registrations.clone(),
                fixture.emitted.production().canonical_shape_definitions(),
                requirements,
                &candidates(&fixture.objects),
            )
            .expect("real ELF type fingerprints");
            assert_eq!(fingerprints.fingerprints().len(), 4);
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
}

fn odr_point(module: &mut Module) {
    let target = module.meta.target_profile;
    let descriptor = module
        .meta
        .type_descriptors
        .iter_mut()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "Point")
        .unwrap()
        .1;
    let root = scoop_lir::MaterializationRoot::lir_structural_odr(descriptor.identity.exact_type())
        .unwrap();
    descriptor.identity =
        scoop_lir::TypeDescriptorIdentity::new(runtime_type("Point"), root.clone()).unwrap();
    descriptor.instance_layout =
        scoop_lir::LayoutIdentity::managed_object(descriptor.identity.exact_type(), target, root)
            .unwrap();
    descriptor.vtable =
        scoop_lir::VtableRecord::new(&descriptor.identity, descriptor.vtable.slots().to_vec())
            .unwrap();
    for itable in &mut descriptor.itables {
        let scoop_identity::OptionalExactInterface::Present(interface) =
            itable.identity_record().key().interface()
        else {
            panic!("itable interface");
        };
        *itable = scoop_lir::ItableRecord::new(
            &descriptor.identity,
            interface,
            itable.interface(),
            itable.slots().to_vec(),
        )
        .unwrap();
    }
}
