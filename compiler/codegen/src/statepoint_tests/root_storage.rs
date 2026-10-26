use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::values::{AsValueRef, BasicValue, CallSiteValue, InstructionOpcode};
use scoop_lir::OptimizationMode;

#[test]
fn only_unexposed_compiler_root_accesses_can_be_promoted() {
    let source = r#"
declare void @poll()
declare void @publish(ptr) #0
declare void @llvm.fake.use(...)
define {ptr addrspace(1), ptr} @f({ptr addrspace(1), ptr} %value) gc "statepoint-example" {
entry:
  %slot = alloca {ptr addrspace(1), ptr}, align 8
  store {ptr addrspace(1), ptr} %value, ptr %slot
  ; exposure
  %object = load volatile ptr addrspace(1), ptr %slot, !scoop.gc-storage-access !1
  call void @poll() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %object) #2, !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %object, ptr %slot, !scoop.gc-storage-access !1
  %after = load volatile ptr addrspace(1), ptr %slot, !scoop.gc-storage-access !1
  %table.slot = getelementptr {ptr addrspace(1), ptr}, ptr %slot, i32 0, i32 1
  %table = load ptr, ptr %table.slot
  %one = insertvalue {ptr addrspace(1), ptr} zeroinitializer, ptr addrspace(1) %after, 0
  %result = insertvalue {ptr addrspace(1), ptr} %one, ptr %table, 1
  ret {ptr addrspace(1), ptr} %result
}
attributes #0 = { "gc-leaf-function" }
attributes #1 = { noduplicate "statepoint-id"="7" }
attributes #2 = { noduplicate }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{}
"#;
    for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
        for (exposure, compiler_accesses, retained) in [
            ("", true, mode == OptimizationMode::Debug),
            ("call void @publish(ptr %slot)", true, true),
            ("", false, true),
        ] {
            let mut source = source.replace("; exposure", exposure);
            if !compiler_accesses {
                source = source.replace(", !scoop.gc-storage-access !1", "");
            }
            let profile =
                ValidatedBackendProfile::darwin_aarch64_for_test().with_optimization(mode);
            let machine = profile.create_target_machine().unwrap();
            let context = Context::create();
            let llvm = parse(&context, &source);
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            crate::statepoint::rewrite(&llvm, &machine, &manifest(Some((7, one_root()))), profile)
                .unwrap();
            llvm.run_passes(
                "function(verify<safepoint-ir>),verify",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
            let ir = llvm.print_to_string().to_string();
            assert_eq!(ir.contains("alloca"), retained, "{ir}");
            assert_eq!(ir.contains("load volatile"), retained, "{ir}");
            assert_eq!(ir.contains("store volatile"), retained, "{ir}");
            assert!(ir.contains("gc.relocate.p1"), "{ir}");
        }
    }
}

#[test]
fn unpromoted_local_memory_receives_relocated_leaves() {
    let context = Context::create();
    let llvm = parse(
        &context,
        r#"
%I = type {ptr addrspace(1), ptr}
declare void @poll()
declare void @llvm.fake.use(...)
define %I @f(%I %first, %I %second, i64 %index) gc "statepoint-example" {
entry:
  %slots = alloca [2 x %I], align 8
  %left = getelementptr [2 x %I], ptr %slots, i64 0, i64 0
  %right = getelementptr [2 x %I], ptr %slots, i64 0, i64 1
  store %I %first, ptr %left
  store %I %second, ptr %right
  %a = load ptr addrspace(1), ptr %left
  %b = load ptr addrspace(1), ptr %right
  call void @poll() #0
  call void (...) @llvm.fake.use(ptr addrspace(1) %a) #1, !scoop.statepoint-root-identity !0
  store volatile ptr addrspace(1) %a, ptr %left, !scoop.gc-storage-access !2
  call void (...) @llvm.fake.use(ptr addrspace(1) %b) #1, !scoop.statepoint-root-identity !1
  store volatile ptr addrspace(1) %b, ptr %right, !scoop.gc-storage-access !2
  %selected = getelementptr [2 x %I], ptr %slots, i64 0, i64 %index
  %result = load %I, ptr %selected
  ret %I %result
}
attributes #0 = { noduplicate "statepoint-id"="7" }
attributes #1 = { noduplicate }
!0 = !{i64 7, i64 0, i64 0, i64 0}
!1 = !{i64 7, i64 0, i64 1, i64 0}
!2 = !{}
"#,
    );
    let profile = ValidatedBackendProfile::darwin_aarch64_for_test()
        .with_optimization(OptimizationMode::Release);
    let machine = profile.create_target_machine().unwrap();
    llvm.set_triple(&machine.get_triple());
    llvm.set_data_layout(&machine.get_target_data().get_data_layout());
    let expected = manifest(Some((
        7,
        ExpectedStatepoint::Relocating(
            [0, 1]
                .map(|index| ExpectedRoot {
                    source: CallerRootSource::Param(index),
                    byte_offset: 0,
                })
                .to_vec()
                .into(),
        ),
    )));
    crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
    let ir = llvm.print_to_string().to_string();
    assert!(ir.contains("alloca [2 x %I]"), "{ir}");
    let stores = llvm
        .get_function("f")
        .unwrap()
        .get_basic_blocks()
        .into_iter()
        .flat_map(|block| block.get_instructions())
        .filter(|instruction| instruction.get_opcode() == InstructionOpcode::Store)
        .filter_map(|instruction| instruction.get_operand(0)?.value()?.as_instruction_value())
        .filter(|value| {
            if value.get_opcode() != InstructionOpcode::Call {
                return false;
            }
            // SAFETY: the store operand is the result of a CallBase.
            unsafe { CallSiteValue::new(value.as_value_ref()) }
                .get_called_fn_value()
                .is_some_and(|callee| {
                    callee
                        .get_name()
                        .to_bytes()
                        .starts_with(b"llvm.experimental.gc.relocate")
                })
        })
        .count();
    assert_eq!(stores, 2, "{ir}");
}
