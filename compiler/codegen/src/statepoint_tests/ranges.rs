use super::*;
use scoop_lir::OptimizationMode;

#[test]
fn dominated_transitive_check_removes_only_the_unreachable_site() {
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let context = Context::create();
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
        let machine = profile.create_target_machine().unwrap();
        let llvm = parse(&context, include_str!("ranges.ll"));
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
        let ir = llvm.print_to_string().to_string();
        assert_eq!(plan.root_count(7), Some(1), "{ir}");
        assert_eq!(
            plan.site_count(),
            if mode == OptimizationMode::Release {
                1
            } else {
                2
            }
        );
        assert_eq!(
            ir.contains("label %failure"),
            mode == OptimizationMode::Debug,
            "{ir}"
        );
        assert!(ir.contains("load volatile ptr addrspace(1)"), "{ir}");
        assert!(ir.contains("gc.relocate.p1"), "{ir}");
    }
}

#[test]
fn nogc_range_propagation_removes_a_redundant_signed_shift() {
    let source = r#"
define i64 @f(i64 %x) {
entry:
  %positive = icmp sge i64 %x, 0
  br i1 %positive, label %bounded, label %outside
bounded:
  %small = icmp ult i64 %x, 256
  br i1 %small, label %inside, label %outside
inside:
  %shift = ashr i64 %x, 8
  ret i64 %shift
outside:
  ret i64 -1
}
"#;
    let context = Context::create();
    let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
        .with_optimization(OptimizationMode::Release);
    let machine = profile.create_target_machine().unwrap();
    let llvm = parse(&context, source);
    let mut expected = manifest(None);
    Arc::make_mut(&mut expected.functions).insert("f".into(), GcEffect::NoGc);
    crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    assert!(!ir.contains("ashr") && !ir.contains("lshr"), "{ir}");
}
