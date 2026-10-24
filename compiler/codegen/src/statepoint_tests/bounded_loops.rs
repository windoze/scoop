use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[path = "bounded_loops/conditional.rs"]
mod conditional;

#[path = "bounded_loops/card_regions.rs"]
mod card_regions;

#[test]
fn bounded_integer_inner_vectorizes_without_copying_outer_sites_or_root_restores() {
    vectorized(include_str!("bounded_loops.ll"), None, "mul <");
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
    vectorized(&source, Some("i32"), "mul <");
    let inside = source
        .replace("  %array = load ptr addrspace(1), ptr %slot, align 8\n", "")
        .replace(
            "  %element = getelementptr",
            "  %array = load ptr addrspace(1), ptr %slot, align 8\n  %element = getelementptr",
        );
    vectorized(&inside, Some("i32"), "mul <");
}

#[test]
fn bounded_float_arrays_vectorize_without_relaxing_rounding_or_root_contracts() {
    for (element, alignment) in [("float", 4), ("double", 8)] {
        let mut source = include_str!("bounded_loops.ll").to_owned();
        let begin = source.find("  %scaled").unwrap();
        let end = source.find("  %next").unwrap();
        source.replace_range(
            begin..end,
            &format!(
                "  %array = load ptr addrspace(1), ptr %slot, align 8\n\
             %element = getelementptr {element}, ptr addrspace(1) %array, i32 %i\n\
             %loaded = load {element}, ptr addrspace(1) %element, align {alignment}\n\
             %scaled = fmul {element} %loaded, 1.5\n\
             %biased = fadd {element} %scaled, 0.25\n\
             store {element} %biased, ptr addrspace(1) %element, align {alignment}\n\
             %accumulated = add i32 %sum, %i\n"
            ),
        );
        vectorized(&source, Some(element), "fmul <");
    }
}

#[test]
fn vectorization_hints_do_not_authorize_reassociation_of_strict_reductions() {
    for element in ["float", "double"] {
        let mut source = include_str!("bounded_loops.ll")
            .replace("define i32 @f", &format!("define {element} @f"))
            .replace(
                "%initial = phi i32 [0,",
                &format!("%initial = phi {element} [0.0,"),
            )
            .replace("%sum = phi i32", &format!("%sum = phi {element}"))
            .replace("%field = load i32", &format!("%field = load {element}"))
            .replace("%result = add i32", &format!("%result = fadd {element}"))
            .replace("ret i32 %result", &format!("ret {element} %result"));
        let begin = source.find("  %scaled").unwrap();
        let end = source.find("  %next").unwrap();
        source.replace_range(
            begin..end,
            &format!(
                "  %array = load ptr addrspace(1), ptr %slot, align 8\n\
             %element = getelementptr {element}, ptr addrspace(1) %array, i32 %i\n\
             %loaded = load {element}, ptr addrspace(1) %element, align 4\n\
             %accumulated = fadd {element} %sum, %loaded\n"
            ),
        );
        validate(&source, |ir| {
            // This pure recurrence has no independently vectorizable adds.
            // Ordered vector.reduce.fadd or the original scalar loop is legal.
            assert!(!ir.contains("fadd <"), "{ir}");
            assert!(ir.contains(&format!("fadd {element}")), "{ir}");
        });
    }
}

fn vectorized(source: &str, arrays: Option<&str>, arithmetic: &str) {
    validate(source, |ir| {
        assert!(ir.contains(arithmetic), "{ir}");
        if let Some(element) = arrays {
            assert!(
                ir.lines().any(|line| line.contains("load <")
                    && line.contains(&format!(" x {element}>, ptr addrspace(1)"))),
                "{ir}"
            );
            assert!(
                ir.lines().any(
                    |line| line.contains("store <") && line.contains(&format!(" x {element}>"))
                ),
                "{ir}"
            );
        }
    });
}

fn validate(source: &str, inspect: impl Fn(&str)) {
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
        inspect(&ir);
        for flag in [
            "fast", "reassoc", "contract", "nnan", "ninf", "nsz", "afn", "arcp",
        ] {
            assert!(!ir.split_whitespace().any(|token| token == flag), "{ir}");
        }
        assert!(
            !ir.contains("@llvm.fma") && !ir.contains("@llvm.fmuladd"),
            "{ir}"
        );
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
