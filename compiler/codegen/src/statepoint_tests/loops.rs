use super::*;
use scoop_lir::OptimizationMode;

#[test]
fn only_typed_no_gc_release_loops_are_vectorized() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            for effect in [GcEffect::NoGc, GcEffect::Managed] {
                let profile = ValidatedBackendProfile::for_selection(selection)
                    .unwrap()
                    .with_optimization(mode);
                let machine = profile.create_target_machine().unwrap();
                let context = Context::create();
                let llvm = parse(&context, include_str!("loops.ll"));
                llvm.set_triple(&machine.get_triple());
                llvm.set_data_layout(&machine.get_target_data().get_data_layout());
                let mut expected = manifest(None);
                Arc::make_mut(&mut expected.functions).insert("f".into(), effect);
                let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
                llvm.verify().unwrap();
                assert_eq!(plan.site_count(), 0);
                let ir = llvm.print_to_string().to_string();
                assert_eq!(
                    ir.contains("mul <4 x i32>"),
                    mode == OptimizationMode::Release && effect == GcEffect::NoGc,
                    "{selection:?}/{mode:?}/{effect:?}: {ir}"
                );
                machine
                    .write_to_memory_buffer(&llvm, inkwell::targets::FileType::Object)
                    .unwrap();
            }
        }
    }
}

#[test]
fn release_reassociates_integers_but_preserves_strict_float_operations() {
    let source = r#"
define i64 @integer(i64 %x, i64 %y, i64 %z) {
  %xy = add i64 %x, %y
  %yz = add i64 %y, %z
  %left = add i64 %xy, %z
  %right = add i64 %yz, %x
  %difference = sub i64 %left, %right
  ret i64 %difference
}
define double @floating(double %x, double %y, double %z) {
  %xy = fadd double %x, %y
  %yz = fadd double %y, %z
  %left = fadd double %xy, %z
  %right = fadd double %yz, %x
  %difference = fsub double %left, %right
  ret double %difference
}
"#;
    for effect in [GcEffect::NoGc, GcEffect::Managed] {
        let context = Context::create();
        let llvm = parse(&context, source);
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
            .with_optimization(OptimizationMode::Release);
        let machine = profile.create_target_machine().unwrap();
        let expected = ExpectedSafepoints {
            sites: BTreeMap::new(),
            functions: BTreeMap::from([("integer".into(), effect), ("floating".into(), effect)])
                .into(),
        };
        crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
        llvm.verify().unwrap();
        let ir = llvm.print_to_string().to_string();
        assert!(ir.contains("ret i64 0"), "{ir}");
        assert_eq!(ir.matches("fadd double").count(), 4, "{ir}");
        assert!(ir.contains("fsub double"), "{ir}");
        assert!(!ir.contains("fast "), "{ir}");
    }
}
