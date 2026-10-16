//! Type metadata produced by LLVM, consumed through the public slib reader.

use scoop_slib::*;

use super::slib_support::{SlibObjects, candidates};
use super::*;

#[test]
fn elf_type_registrations_hash_descriptor_diagnostic_and_itable_data() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for odr in [false, true] {
            let mut module = super::super::objects::classes_module_for(
                scoop_target_data::layout(target)
                    .expect("implemented target selected by this consumer"),
            );
            if odr {
                odr_point(&mut module);
            }
            let fixture = SlibObjects::new(module, directory.path());
            let sites = fixture.sites(fixture.builtins(&fixture.objects));
            let data = bind_type_registrations_v1(
                sites,
                fixture
                    .emitted
                    .production()
                    .registration_production()
                    .types()
                    .clone(),
            )
            .unwrap();
            assert_eq!(data.registrations().len(), 4);
            assert!(data.plan().registrations().iter().any(|plan| matches!(
                plan.itable_directory(),
                scoop_lir::TypeDescriptorITableDirectoryV1::Defined(_)
            )));
            let fingerprints = compute_strong_type_fingerprints_v1(
                data,
                fixture.emitted.production().canonical_shape_definitions(),
                &candidates(&fixture.objects),
            )
            .unwrap();
            assert_eq!(fingerprints.fingerprints().len(), 4);
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
