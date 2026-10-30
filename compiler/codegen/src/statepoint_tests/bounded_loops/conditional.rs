use super::*;

fn source() -> String {
    let mut source = include_str!("../bounded_loops.ll").replace(", %inner]", ", %latch]");
    let start = source.find("  %scaled").unwrap();
    let end = source.find("  %next").unwrap();
    source.replace_range(
        start..end,
        r#"  %array = load ptr addrspace(1), ptr %slot, align 8
  %element = getelementptr i32, ptr addrspace(1) %array, i32 %i
  %loaded = load i32, ptr addrspace(1) %element, align 4
  %condition = icmp slt i32 %loaded, 0
  br i1 %condition, label %negative, label %positive
negative:
  %scaled = mul i32 %loaded, 1664525
  br label %latch
positive:
  %biased = add i32 %loaded, 1013904223
  br label %latch
latch:
  %mixed = phi i32 [%scaled, %negative], [%biased, %positive]
  %accumulated = add i32 %sum, %mixed
  store i32 %mixed, ptr addrspace(1) %element, align 4
"#,
    );
    source
}

#[test]
fn bounded_conditional_arrays_vectorize_and_preserve_outer_root_sites() {
    vectorized(&source(), Some("i32"), "mul <");
}

#[test]
fn an_inner_cycle_early_exit_or_conditional_root_write_cannot_bypass_the_bound() {
    let original = source();
    let early = original
        .replace(
            "[0, %entry], [%next, %latch]",
            "[0, %entry], [%next, %latch], [%i, %negative]",
        )
        .replace(
            "[0, %entry], [%accumulated, %latch]",
            "[0, %entry], [%accumulated, %latch], [%sum, %negative]",
        )
        .replacen(
            "  br label %latch",
            "  br i1 %condition, label %outer, label %latch",
            1,
        );
    for source in [
        early,
        original.replacen(
            "  br label %latch",
            "  br i1 %condition, label %negative, label %latch",
            1,
        ),
        original.replace(
            "negative:\n",
            "negative:\n  store ptr addrspace(1) %root, ptr %slot, align 8\n",
        ),
        original.replace(
            "positive:\n",
            "positive:\n  %effect = load volatile i32, ptr addrspace(1) %root, align 4\n",
        ),
    ] {
        let context = Context::create();
        let llvm = parse(&context, &source);
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test();
        let machine = profile.create_target_machine().unwrap();
        crate::statepoint::bounded_loops::optimize(
            llvm.get_function("f").unwrap(),
            &machine,
            ValidatedBackendProfile::darwin_aarch64_for_test().tuning(),
        )
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
