use super::*;
use scoop_lir::OptimizationMode;

const GROUPED_ROOTS: &str = r#"
declare void @callee()
declare void @llvm.fake.use(...)
define void @f(ptr addrspace(1) %a, ptr addrspace(1) %b) #0 gc "statepoint-example" {
entry:
  call void @callee() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %a), !scoop.statepoint-root-identity !0
  call void (...) @llvm.fake.use(ptr addrspace(1) %a), !scoop.statepoint-root-identity !1
  call void (...) @llvm.fake.use(ptr addrspace(1) %b), !scoop.statepoint-root-identity !2
  call void (...) @llvm.fake.use(ptr addrspace(1) null), !scoop.statepoint-root-identity !3
  ret void
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 7, i64 1, i64 0, i64 0}
!2 = !{i64 7, i64 0, i64 1, i64 0}
!3 = !{i64 7, i64 1, i64 1, i64 0}
"#;

fn local(index: u32) -> CallerRootSource {
    CallerRootSource::Local(la_arena::Idx::from_raw(la_arena::RawIdx::from_u32(index)))
}

fn logical_roots() -> ExpectedSafepoints {
    let roots = [
        CallerRootSource::Param(0),
        local(0),
        CallerRootSource::Param(1),
        local(1),
    ]
    .map(|source| ExpectedRoot {
        source,
        byte_offset: 0,
    });
    manifest(Some((
        7,
        ExpectedStatepoint::Relocating(roots.to_vec().into()),
    )))
}

#[test]
fn final_ssa_roots_merge_aliases_and_exclude_null_without_identity_allocas() {
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, GROUPED_ROOTS);
        let initial = logical_roots();
        assert_eq!(initial.root_count(7), Some(4));
        let plan = crate::statepoint::rewrite(&llvm, &machine, &initial, profile).unwrap();
        assert_eq!(plan.root_count(7), Some(2));

        llvm.verify().unwrap();
        let ir = llvm.print_to_string().to_string();
        assert!(!ir.contains("alloca"), "{mode:?}: {ir}");
        assert!(ir.contains("@llvm.fake.use(ptr addrspace(1) null)"), "{ir}");
    }
}

#[test]
fn release_simplifies_redundant_integer_expressions() {
    let source = r#"
define i64 @f(i64 %value) #0 gc "statepoint-example" {
entry:
  %first = add i64 %value, 3
  %second = add i64 %value, 3
  %difference = sub i64 %first, %second
  ret i64 %difference
}


attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
"#;
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, source);
        crate::statepoint::rewrite(&llvm, &machine, &manifest(None), profile).unwrap();

        let ir = llvm.print_to_string().to_string();
        assert_eq!(
            ir.contains("ret i64 0"),
            mode == OptimizationMode::Release,
            "{ir}"
        );
        assert_eq!(
            ir.contains("add i64"),
            mode == OptimizationMode::Debug,
            "{ir}"
        );
    }
}

#[test]
fn terminal_managed_call_keeps_its_root_marker_before_the_volatile_restoration() {
    let source = r#"
declare void @callee(ptr addrspace(1))
declare void @llvm.fake.use(...)
define void @f(ptr addrspace(1) %root) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  call void @callee(ptr addrspace(1) %root) #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %root), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %root, ptr %slot
  unreachable
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#;
    let context = Context::create();
    let llvm = parse(&context, source);
    let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
        .with_optimization(OptimizationMode::Release);
    let machine = profile.create_target_machine().unwrap();
    let plan =
        crate::statepoint::rewrite(&llvm, &machine, &manifest(Some((7, one_root()))), profile)
            .unwrap();
    assert_eq!(plan.root_count(7), Some(1));
}

#[test]
fn roots_exposed_by_dead_store_removal_match_machine_coalescing() {
    let source = r#"
declare void @callee()
declare void @llvm.fake.use(...)
define void @f(ptr %source) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  %first = load ptr addrspace(1), ptr %source, align 8
  store ptr addrspace(1) %first, ptr %slot, align 8
  %second = load ptr addrspace(1), ptr %source, align 8
  call void @callee() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %first), !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %first, ptr %slot, align 8
  call void (...) @llvm.fake.use(ptr addrspace(1) %second), !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %second, ptr %source, align 8
  ret void
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 1, i64 0, i64 0}
!1 = !{i64 7, i64 1, i64 1, i64 0}
"#;
    let directory = tempfile::tempdir().unwrap();
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, source);
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let roots = [local(0), local(1)].map(|source| ExpectedRoot {
            source,
            byte_offset: 0,
        });
        let plan = crate::statepoint::rewrite(
            &llvm,
            &machine,
            &manifest(Some((
                7,
                ExpectedStatepoint::Relocating(roots.to_vec().into()),
            ))),
            profile,
        )
        .unwrap();

        let output = directory.path().join(format!("{mode:?}.o"));
        machine
            .write_to_file(&llvm, inkwell::targets::FileType::Object, &output)
            .unwrap();
        assert_machine_site_count(&output, plan.site_count());
    }
}
