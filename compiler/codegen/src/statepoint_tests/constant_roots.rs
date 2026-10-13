use super::*;
use scoop_lir::OptimizationMode;

#[test]
fn constant_pointer_selections_do_not_relocate_but_dynamic_inputs_do() {
    for selection in [
        "%value = select i1 %condition, ptr addrspace(1) FIRST, ptr addrspace(1) OTHER",
        "br i1 %condition, label %left, label %right\nleft:\n br label %join\nright:\n br label %join\njoin:\n %value = phi ptr addrspace(1) [FIRST, %left], [OTHER, %right]",
    ] {
        for (first, other, dynamic) in [
            ("@first", "@second", false),
            ("null", "%input", true),
            ("@first", "%input", true),
        ] {
            let selection = selection.replace("FIRST", first).replace("OTHER", other);
            let source = format!(
                r#"
@first = addrspace(1) constant i8 0
@second = addrspace(1) constant i8 1
declare void @callee()
declare void @llvm.fake.use(...)
define void @f(i1 %condition, ptr addrspace(1) %input) #0 gc "statepoint-example" {{
entry:
  {selection}
  call void @callee() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %value), !scoop.statepoint-root-identity !0
  ret void
}}
attributes #0 = {{ "disable-tail-calls"="true" "frame-pointer"="all" }}
attributes #1 = {{ "statepoint-id"="7" }}
!0 = !{{i64 7, i64 0, i64 0, i64 0}}
"#
            );
            for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
                let profile =
                    ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
                let machine = profile.create_target_machine().unwrap();
                let context = Context::create();
                let llvm = parse(&context, &source);
                let initial = manifest(Some((
                    7,
                    ExpectedStatepoint::Relocating(
                        vec![ExpectedRoot {
                            source: CallerRootSource::Param(0),
                            byte_offset: 0,
                        }]
                        .into(),
                    ),
                )));
                let plan = crate::statepoint::rewrite(&llvm, &machine, &initial, profile).unwrap();
                assert_eq!(plan.root_count(7), Some(usize::from(dynamic)));
                verify_rewritten_with_profile(&llvm, &plan, profile).unwrap();
            }
        }
    }
}

#[test]
fn a_derived_address_hidden_in_a_root_merge_is_rejected() {
    let context = Context::create();
    let llvm = parse(
        &context,
        r#"
@first = addrspace(1) constant i8 0
declare void @callee()
declare void @llvm.fake.use(...)
define void @f(i1 %condition, ptr addrspace(1) %input) #0 gc "statepoint-example" {
entry:
  %derived = getelementptr i8, ptr addrspace(1) %input, i64 8
  %value = select i1 %condition, ptr addrspace(1) @first, ptr addrspace(1) %derived
  call void @callee() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %value), !scoop.statepoint-root-identity !0
  ret void
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#,
    );
    let profile = ValidatedBackendProfile::darwin_aarch64_for_test();
    let machine = profile.create_target_machine().unwrap();
    let error =
        crate::statepoint::rewrite(&llvm, &machine, &manifest(Some((7, one_root()))), profile)
            .unwrap_err();
    assert!(error.0.contains("derived address"), "{error:?}");
}
