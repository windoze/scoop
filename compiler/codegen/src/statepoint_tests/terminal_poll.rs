use super::*;
use inkwell::targets::FileType;
use scoop_lir::OptimizationMode;

#[test]
fn terminal_call_after_conditional_poll_keeps_one_machine_site() {
    let directory = tempfile::tempdir().unwrap();
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target)
                    .expect("implemented target selected by this consumer"),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, include_str!("terminal_poll.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(
                &profile
                    .managed_address_space_contract()
                    .data_layout(&machine.get_target_data()),
            );
            crate::statepoint::configure_function(
                &context,
                llvm.get_function("f").unwrap(),
                GcEffect::Managed,
                profile,
            );
            let expected = ExpectedSafepoints {
                sites: [(7, "poll.slow"), (8, "poll.continue")]
                    .into_iter()
                    .map(|(id, block)| {
                        (
                            id,
                            ExpectedSite {
                                function: "f".into(),
                                block: block.into(),
                                statepoint: ExpectedStatepoint::Relocating(Vec::new().into()),
                            },
                        )
                    })
                    .collect(),
                functions: BTreeMap::from([("f".into(), GcEffect::Managed)]).into(),
            };
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();

            assert_eq!(plan.root_count(7), Some(0));
            assert_eq!(plan.root_count(8), Some(0));
            let output = directory
                .path()
                .join(format!("{}-{mode:?}.o", target.canonical_name()));
            machine
                .write_to_file(&llvm, FileType::Object, &output)
                .unwrap();
            assert_machine_site_count(&output, plan.site_count());
        }
    }
}

#[test]
fn terminal_poll_with_discarded_restorations_matches_machine_roots() {
    // A generated continuation adapter for an uninhabited result can retain its
    // entry poll while all later field reads and root restorations become dead.
    let source = r#"
@phase = external global i32
declare void @tick() nounwind
declare void @llvm.fake.use(...)
define void @f(ptr addrspace(1) %root) #0 gc "statepoint-example" {
entry:
  %slot = alloca ptr addrspace(1), align 8
  store ptr addrspace(1) %root, ptr %slot, align 8
  %state = load atomic i32, ptr @phase acquire, align 4
  %running = icmp eq i32 %state, 0
  br i1 %running, label %next, label %slow
slow:
  %before = load ptr addrspace(1), ptr %slot, align 8
  call void @tick() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %before) #2, !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %before, ptr %slot, align 8, !scoop.gc-storage-access !1
  br label %next
next:
  %after = load ptr addrspace(1), ptr %slot, align 8
  %field = getelementptr i8, ptr addrspace(1) %after, i64 16
  %value = load ptr addrspace(1), ptr addrspace(1) %field, align 8
  unreachable
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { noduplicate "statepoint-id"="7" }
attributes #2 = { noduplicate }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{}
"#;
    let directory = tempfile::tempdir().unwrap();
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        let profile = ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
        let machine = profile.create_target_machine().unwrap();
        let context = Context::create();
        let llvm = parse(&context, source);
        llvm.set_triple(&machine.get_triple());
        llvm.set_data_layout(&machine.get_target_data().get_data_layout());
        let plan =
            crate::statepoint::rewrite(&llvm, &machine, &manifest(Some((7, one_root()))), profile)
                .unwrap();
        llvm.verify().unwrap();
        assert_eq!(plan.site_count(), 1);
        assert_eq!(
            plan.root_count(7),
            Some(usize::from(mode == OptimizationMode::Debug))
        );
        let output = directory.path().join(format!("{mode:?}.o"));
        machine
            .write_to_file(&llvm, FileType::Object, &output)
            .unwrap();
        assert_machine_root_counts(&output, &plan);
    }
}

#[test]
fn missing_restoration_on_a_returning_call_remains_an_error() {
    let context = Context::create();
    let llvm = parse(
        &context,
        r#"
declare void @tick()
define void @f(ptr addrspace(1) %root) gc "statepoint-example" {
entry:
  call void @tick() #0
  ret void
}
attributes #0 = { noduplicate "statepoint-id"="7" }
"#,
    );
    let error = crate::statepoint::finalization::finalize(
        &llvm,
        &manifest(Some((7, one_root()))),
        ValidatedBackendProfile::darwin_aarch64_for_test(),
    )
    .unwrap_err();
    assert!(
        error.0.contains("incomplete typed leaf restorations"),
        "{error}"
    );
}
