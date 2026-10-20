use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn late_load_elimination_prunes_sites_before_finalizing_roots() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, include_str!("late_branches.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            crate::statepoint::configure_function(
                &context,
                llvm.get_function("f").unwrap(),
                GcEffect::Managed,
                profile,
            );
            let mut expected = manifest(Some((7, one_root())));
            expected.sites.insert(
                8,
                ExpectedSite {
                    function: "f".into(),
                    block: "failure".into(),
                    statepoint: one_root(),
                },
            );
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            llvm.verify().unwrap();
            assert_eq!(plan.root_count(7), Some(1));
            assert_eq!(
                plan.root_count(8),
                (mode == OptimizationMode::Debug).then_some(1)
            );
            let ir = llvm.print_to_string().to_string();
            assert!(ir.contains("load volatile ptr addrspace(1)"), "{ir}");
            assert!(ir.contains("gc.relocate.p1"), "{ir}");
            let output = directory
                .path()
                .join(format!("{}-{mode:?}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_site_count(&output, plan.site_count());
        }
    }
}
