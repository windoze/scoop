use super::*;

fn kernel() -> String {
    let mut source = include_str!("../bounded_loops.ll").to_owned();
    let begin = source.find("  %scaled").unwrap();
    let end = source.find("  %next").unwrap();
    source.replace_range(
        begin..end,
        "  %array = load ptr addrspace(1), ptr %slot, align 8\n\
         %element = getelementptr ptr addrspace(1), ptr addrspace(1) %array, i32 %i\n\
         %node = load ptr addrspace(1), ptr addrspace(1) %element, align 8\n\
         %value = load i32, ptr addrspace(1) %node, align 4\n\
         %accumulated = add i32 %sum, %value\n",
    );
    source
}

#[test]
fn reference_read_unrolling_preserves_outer_sites_and_relocated_roots() {
    for count in [4, 8, 64] {
        let source = kernel()
            .replace("[64, %chunk]", &format!("[{count}, %chunk]"))
            .replace("icmp slt i32 %start, %count", "icmp slt i32 %start, 128");
        validate(&source, |ir| {
            assert_eq!(ir.matches("load i32, ptr addrspace(1)").count(), 5, "{ir}");
            assert!(!ir.contains("load <") && !ir.contains("phi <"), "{ir}");
        });
    }
}

#[test]
fn reference_reads_do_not_relax_float_reduction_order() {
    let source = kernel()
        .replace("define i32 @f", "define double @f")
        .replace("%initial = phi i32 [0,", "%initial = phi double [0.0,")
        .replace("%sum = phi i32", "%sum = phi double")
        .replace(
            "load i32, ptr addrspace(1)",
            "load double, ptr addrspace(1)",
        )
        .replace("%accumulated = add i32", "%accumulated = fadd double")
        .replace("%result = add i32", "%result = fadd double")
        .replace("ret i32 %result", "ret double %result");
    validate(&source, |ir| {
        assert_eq!(ir.matches("fadd double").count(), 5, "{ir}");
        let additions = ir
            .lines()
            .filter(|line| line.contains("fadd double"))
            .take(4)
            .collect::<Vec<_>>();
        for pair in additions.windows(2) {
            let previous = pair[0].split('=').next().unwrap().trim();
            assert!(pair[1].contains(&format!("{previous},")), "{ir}");
        }
    });
}

#[test]
fn reference_unrolling_rejects_writes_effects_and_nondivisible_counts() {
    let source = kernel();
    for source in [
        source.replace("[64, %chunk]", "[2, %chunk]"),
        source.replace("[64, %chunk]", "[5, %chunk]"),
        source.replace("[64, %chunk]", "[65, %chunk]"),
        source.replace("[64, %chunk]", "[%count, %chunk]"),
        source.replace("%value = load i32", "%value = load volatile i32"),
        source.replace(
            "%value = load i32, ptr addrspace(1) %node, align 4",
            "%value = load atomic i32, ptr addrspace(1) %node acquire, align 4",
        ),
        source.replace("%array = load ptr", "%array = load volatile ptr"),
        source.replace(
            "outer:\n",
            "outer:\n  %last = phi ptr addrspace(1) [%root, %entry], [%node, %inner]\n",
        ),
        source.replace(
            "  %next = add",
            "  store i32 %value, ptr addrspace(1) %node, align 4\n  %next = add",
        ),
        source.replace("  %next = add", "  call void @tick()\n  %next = add"),
        source
            .replace(
                "  %next = add",
                "  %found = icmp eq i32 %value, 0\n\
             br i1 %found, label %done, label %latch\n\
             done:\n  ret i32 %accumulated\n\
             latch:\n  %next = add",
            )
            .replace("%inner]", "%latch]"),
    ] {
        let context = Context::create();
        let llvm = parse(&context, &source);
        llvm.verify().unwrap();
        let before = llvm.print_to_string().to_string();
        let machine = ValidatedBackendProfile::darwin_aarch64_for_test()
            .create_target_machine()
            .unwrap();
        crate::statepoint::bounded_loops::optimize(llvm.get_function("f").unwrap(), &machine)
            .unwrap();
        assert_eq!(llvm.print_to_string().to_string(), before);
    }
}
