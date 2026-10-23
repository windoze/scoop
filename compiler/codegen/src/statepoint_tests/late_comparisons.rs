use super::*;
use inkwell::targets::FileType;

#[test]
fn final_cleanup_consumes_newly_equal_dynamic_bounds() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        let profile =
            ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
                .unwrap()
                .with_optimization(scoop_lir::OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        // This is the SSA shape after late load elimination has made the two
        // bounds equal. Final cleanup must run before recording any roots.
        let llvm = parse(&context, include_str!("late_comparisons.ll"));
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(
            &profile
                .managed_address_space_contract()
                .data_layout(&machine.get_target_data()),
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
        crate::statepoint::policy::finish_managed_optimization(
            llvm.get_function("f").unwrap(),
            &machine,
        )
        .unwrap();
        crate::statepoint::finalization::finalize(&llvm, &expected, profile).unwrap();
        let plan = crate::statepoint::lower(&llvm, &machine).unwrap();
        assert_eq!(plan.root_count(7), Some(1));
        assert_eq!(plan.root_count(8), None);
        llvm.verify().unwrap();
        let output = directory
            .path()
            .join(format!("{}.o", target.canonical_name()));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .unwrap();
        assert_machine_root_counts(&output, &plan);
    }
}
