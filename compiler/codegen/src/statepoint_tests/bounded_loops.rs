use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn bounded_integer_inner_vectorizes_without_copying_outer_sites_or_root_restores() {
    validate(include_str!("bounded_loops.ll"), false);
}

#[test]
fn bounded_array_inner_keeps_relocated_bases_and_scalar_root_restores() {
    let source = include_str!("bounded_loops.ll")
        .replace(
            "chunk:\n  br label %inner",
            "chunk:\n  %array = load ptr addrspace(1), ptr %slot, align 8\n  br label %inner",
        )
        .replace(
            "  %scaled = mul i32 %i, 1664525",
            "  %element = getelementptr i32, ptr addrspace(1) %array, i32 %i\n  %loaded = load i32, ptr addrspace(1) %element, align 4\n  %scaled = mul i32 %loaded, 1664525",
        )
        .replace(
            "  %next = add i32 %i, 1",
            "  store i32 %mixed, ptr addrspace(1) %element, align 4\n  %next = add i32 %i, 1",
        );
    validate(&source, true);
    let inside = source
        .replace("  %array = load ptr addrspace(1), ptr %slot, align 8\n", "")
        .replace(
            "  %element = getelementptr",
            "  %array = load ptr addrspace(1), ptr %slot, align 8\n  %element = getelementptr",
        );
    validate(&inside, true);
}

fn validate(source: &str, arrays: bool) {
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
        let llvm = parse(&context, source);
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
        if arrays {
            assert!(
                ir.contains("load <4 x i32>, ptr addrspace(1)"),
                "{selection:?}: {ir}"
            );
            assert!(ir.contains("store <4 x i32>"), "{selection:?}: {ir}");
        }
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
fn variable_count_and_unsafe_memory_effects_do_not_enable_managed_vectorization() {
    let source = include_str!("bounded_loops.ll");
    for source in [
        source.replace("[64, %chunk]", "[%count, %chunk]"),
        source.replace("[64, %chunk]", "[65, %chunk]"),
        source.replace(
            "%scaled = mul",
            "%effect = load atomic i32, ptr addrspace(1) %root acquire, align 4\n  %scaled = mul",
        ),
        source.replace(
            "%scaled = mul",
            "%effect = load volatile i32, ptr addrspace(1) %root, align 4\n  %scaled = mul",
        ),
        source.replace(
            "%scaled = mul",
            "store volatile i32 %i, ptr addrspace(1) %root, align 4\n  %scaled = mul",
        ),
        source.replace(
            "%scaled = mul",
            "%effect = load volatile ptr addrspace(1), ptr %slot, align 8\n  %scaled = mul",
        ),
        source.replace(
            "%scaled = mul",
            "store ptr addrspace(1) %root, ptr addrspace(1) %root, align 8\n  %scaled = mul",
        ),
        source.replace(
            "%scaled = mul",
            "%raw = addrspacecast ptr addrspace(1) %root to ptr\n  store i32 %i, ptr %raw, align 4\n  %scaled = mul",
        ),
        source.replace("  br label %outer", "  call void (...) @llvm.fake.use(ptr %slot)\n  br label %outer").replace(
            "%scaled = mul", "%effect = load ptr addrspace(1), ptr %slot, align 8\n  %scaled = mul",
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
