use super::*;
use inkwell::targets::FileType;
use inkwell::values::{AsValueRef, CallSiteValue, InstructionOpcode};
use scoop_lir::OptimizationMode;

#[test]
fn dead_direct_arguments_keep_machine_roots_without_relocation_uses() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(
                &context,
                r#"
declare void @consume(ptr addrspace(1), ptr)
define void @f(ptr addrspace(1) %object, ptr %itab) gc "statepoint-example" {
entry:
  call void @consume(ptr addrspace(1) %object, ptr %itab) #0
  ret void
}

attributes #0 = { noduplicate "statepoint-id"="7" }
"#,
            );
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let expected = manifest(Some((7, ExpectedStatepoint::Relocating(Vec::new().into()))));
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            llvm.verify().unwrap();
            assert_eq!(plan.root_count(7), Some(1));
            let function = llvm.get_function("f").unwrap();
            let relocated = function
                .get_first_basic_block()
                .unwrap()
                .get_instructions()
                .find(|instruction| {
                    if instruction.get_opcode() != InstructionOpcode::Call {
                        return false;
                    }
                    // SAFETY: the opcode establishes a CallBase, including
                    // tokens that cannot be converted to BasicValueEnum.
                    unsafe { CallSiteValue::new(instruction.as_value_ref()) }
                        .get_called_fn_value()
                        .is_some_and(|callee| {
                            callee
                                .get_name()
                                .to_bytes()
                                .starts_with(b"llvm.experimental.gc.relocate")
                        })
                })
                .unwrap();
            assert!(relocated.get_first_use().is_none());
            let directory = tempfile::tempdir().unwrap();
            let object = directory.path().join("dead-argument.o");
            machine
                .write_to_file(&llvm, FileType::Object, &object)
                .unwrap();
            assert_machine_root_counts(&object, &plan);
        }
    }
}

#[test]
fn duplicate_argument_and_continuation_loads_share_one_physical_root() {
    for target in [
        scoop_target_data::DARWIN_AARCH64_ID,
        scoop_target_data::LINUX_X86_64_GNU_ID,
        scoop_target_data::LINUX_X86_64_MUSL_ID,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            let profile = ValidatedBackendProfile::for_selection(
                scoop_target_data::selection(target).unwrap(),
            )
            .unwrap()
            .with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(
                &context,
                r#"
declare void @consume(ptr addrspace(1))
declare void @llvm.fake.use(...)
define void @f(ptr %storage) gc "statepoint-example" {
entry:
  %argument = load ptr addrspace(1), ptr %storage
  %continuation = load ptr addrspace(1), ptr %storage
  call void @consume(ptr addrspace(1) %argument) #0
  call void (...) @llvm.fake.use(ptr addrspace(1) %continuation) #1, !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %continuation, ptr %storage
  ret void
}
attributes #0 = { noduplicate "statepoint-id"="7" }
attributes #1 = { noduplicate }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#,
            );
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let plan = crate::statepoint::rewrite(
                &llvm,
                &machine,
                &manifest(Some((7, one_root()))),
                profile,
            )
            .unwrap();
            assert_eq!(plan.root_count(7), Some(1));
            let directory = tempfile::tempdir().unwrap();
            let object = directory.path().join("shared-argument.o");
            machine
                .write_to_file(&llvm, FileType::Object, &object)
                .unwrap();
            assert_machine_root_counts(&object, &plan);
        }
    }
}
