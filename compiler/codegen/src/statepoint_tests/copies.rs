use super::*;
use scoop_lir::OptimizationMode;

const COPY: &str = r#"
declare void @fill(ptr) "gc-leaf-function"
declare void @mutate(ptr) "gc-leaf-function"
declare void @consume(ptr byval([8 x i64]) align 8)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1 immarg)
define void @f() GC {
entry:
  %source = alloca [8 x i64], align 8
  %copy = alloca [8 x i64], align 8
  call void @fill(ptr %source)
  call void @llvm.memcpy.p0.p0.i64(ptr %copy, ptr %source, i64 64, i1 false)
  MUTATE
  call void @consume(ptr byval([8 x i64]) align 8 %copy) ATTR
  ret void
}
attributes #0 = { "statepoint-id"="7" }
"#;

#[test]
fn byval_forwarding_preserves_the_value_before_an_intervening_write() {
    for effect in [GcEffect::NoGc, GcEffect::Managed] {
        for mutation in [false, true] {
            let source = COPY
                .replace(
                    "GC",
                    if effect == GcEffect::Managed {
                        "gc \"statepoint-example\""
                    } else {
                        ""
                    },
                )
                .replace(
                    "ATTR",
                    if effect == GcEffect::Managed {
                        "#0"
                    } else {
                        ""
                    },
                )
                .replace(
                    "MUTATE",
                    if mutation {
                        "call void @mutate(ptr %source)"
                    } else {
                        ""
                    },
                );
            let context = Context::create();
            let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
                .with_optimization(OptimizationMode::Release);
            let machine = profile.create_target_machine().unwrap();
            let llvm = parse(&context, &source);
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            let site = (effect == GcEffect::Managed)
                .then_some((7, ExpectedStatepoint::Relocating(Vec::new().into())));
            let mut expected = manifest(site);
            expected.functions.insert("f".into(), effect);
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            llvm.verify().unwrap();
            let ir = llvm.print_to_string().to_string();
            assert_eq!(
                ir.contains("call void @llvm.memcpy"),
                mutation,
                "{effect:?}: {ir}"
            );
            assert_eq!(plan.site_count(), usize::from(effect == GcEffect::Managed));
            assert!(ir.contains("byval([8 x i64]) align 8"), "{ir}");
        }
    }
}
