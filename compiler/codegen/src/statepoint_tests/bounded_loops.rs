use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn bounded_integer_inner_vectorizes_without_copying_outer_sites_or_root_restores() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1,
    ] {
        let profile = ValidatedBackendProfile::for_selection(selection)
            .unwrap()
            .with_optimization(OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, include_str!("bounded_loops.ll"));
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let mut expected = manifest(Some((7, one_root())));
        expected.sites.insert(
            8,
            ExpectedSite {
                function: "f".into(),
                block: "outer".into(),
                statepoint: one_root(),
            },
        );
        let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        assert_eq!(plan.site_count(), 2);
        assert_eq!(plan.root_count(7), Some(1));
        assert_eq!(plan.root_count(8), Some(1));
        llvm.run_passes(
            "function(verify<safepoint-ir>),verify",
            &machine,
            PassBuilderOptions::create(),
        )
        .unwrap();
        let ir = llvm.print_to_string().to_string();
        assert!(ir.contains("mul <4 x i32>"), "{selection:?}: {ir}");
        assert_eq!(
            ir.matches("store volatile ptr addrspace(1)").count(),
            2,
            "{ir}"
        );
        assert!(ir.contains("gc.relocate.p1"), "{ir}");
        assert!(!ir.contains("ptrtoint"), "{ir}");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bounded.o");
        machine
            .write_to_file(&llvm, FileType::Object, &path)
            .unwrap();
        assert_machine_site_count(&path, 2);
    }
}

#[test]
fn variable_count_and_memory_effects_do_not_enable_managed_vectorization() {
    let source = include_str!("bounded_loops.ll");
    for source in [
        source.replace("[64, %chunk]", "[%count, %chunk]"),
        source.replace("[64, %chunk]", "[65, %chunk]"),
        source.replace(
            "%scaled = mul",
            "%effect = load atomic i32, ptr addrspace(1) %root acquire, align 4\n  %scaled = mul",
        ),
    ] {
        let context = Context::create();
        let llvm = parse(&context, &source);
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test();
        let machine = profile.create_target_machine().unwrap();
        crate::statepoint::bounded_loops::optimize(llvm.get_function("f").unwrap(), &machine)
            .unwrap();
        llvm.verify().unwrap();
        assert!(
            !llvm
                .print_to_string()
                .to_string()
                .contains("llvm.loop.vectorize.enable")
        );
    }
}
