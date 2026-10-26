use super::*;
use inkwell::values::{BasicValue, BasicValueEnum, InstructionOpcode};
use scoop_lir::OptimizationMode;

const SOURCE: &str = r#"
declare void @callee()
declare void @observe(i64) #1
declare i64 @scoop.gc.address.bits(ptr addrspace(1)) #1
declare void @llvm.fake.use(...)
define i64 @f(ptr addrspace(1) %object, i1 %poll) #0 gc "statepoint-example" {
entry:
  %before = ptrtoint ptr addrspace(1) %object to i64
  %page = lshr i64 %before, 16
  call void @observe(i64 %page)
  %external = call i64 @scoop.gc.address.bits(ptr addrspace(1) %object)
  call void @observe(i64 %external)
  br i1 %poll, label %gc, label %done
gc:
  call void @callee() #2
  call void (...) @llvm.fake.use(ptr addrspace(1) %object), !scoop.statepoint-root-identity !0
  br label %done
done:
  %after = ptrtoint ptr addrspace(1) %object to i64
  %current = lshr i64 %after, 16
  ret i64 %current
}
attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { nounwind "gc-leaf-function" }
attributes #2 = { "statepoint-id"="7" }
!0 = !{i64 7, i64 0, i64 0, i64 0}
"#;

#[test]
fn address_bits_after_a_call_or_conditional_poll_use_the_relocated_object() {
    for selection in [
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1,
    ] {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            for conditional in [false, true] {
                let context = Context::create();
                let profile = ValidatedBackendProfile::for_selection(selection)
                    .unwrap()
                    .with_optimization(mode);
                let machine = profile.create_target_machine().unwrap();
                let source = if conditional {
                    SOURCE.to_owned()
                } else {
                    SOURCE.replace("br i1 %poll, label %gc, label %done", "br label %gc")
                };
                let llvm = parse(&context, &source);
                let plan = crate::statepoint::rewrite(
                    &llvm,
                    &machine,
                    &manifest(Some((7, one_root()))),
                    profile,
                )
                .unwrap();
                assert_eq!(plan.root_count(7), Some(1));
                llvm.verify().unwrap();
                let ir = llvm.print_to_string().to_string();
                let function = llvm.get_function("f").unwrap();
                let result = function
                    .get_basic_blocks()
                    .iter()
                    .flat_map(|block| block.get_instructions())
                    .find(|instruction| instruction.get_opcode() == InstructionOpcode::Return)
                    .unwrap()
                    .get_operand(0)
                    .unwrap()
                    .value()
                    .unwrap();
                let shift = result.as_instruction_value().unwrap();
                assert_eq!(shift.get_opcode(), InstructionOpcode::LShr, "{ir}");
                let conversion = shift
                    .get_operand(0)
                    .unwrap()
                    .value()
                    .unwrap()
                    .as_instruction_value()
                    .unwrap();
                assert_eq!(conversion.get_opcode(), InstructionOpcode::PtrToInt, "{ir}");
                let pointer = conversion.get_operand(0).unwrap().value().unwrap();
                assert!(
                    uses_relocation(pointer),
                    "{mode:?}, conditional={conditional}: {ir}"
                );
                assert!(!ir.contains("\"scoop.gc-address-bits\""), "{ir}");
                // A real extern with the same preferred name remains an ordinary call.
                assert!(ir.contains("call i64 @scoop.gc.address.bits("), "{ir}");
                machine
                    .write_to_memory_buffer(&llvm, inkwell::targets::FileType::Object)
                    .unwrap();
            }
        }
    }
}

fn uses_relocation(value: BasicValueEnum<'_>) -> bool {
    let Some(instruction) = value.as_instruction_value() else {
        return false;
    };
    match instruction.get_opcode() {
        InstructionOpcode::Call => inkwell::values::CallSiteValue::try_from(instruction)
            .ok()
            .and_then(|call| call.get_called_fn_value())
            .is_some_and(|callee| {
                callee
                    .get_name()
                    .to_bytes()
                    .starts_with(b"llvm.experimental.gc.relocate")
            }),
        InstructionOpcode::Phi => instruction
            .get_operands()
            .flatten()
            .filter_map(|operand| operand.value())
            .any(uses_relocation),
        _ => false,
    }
}

#[test]
fn address_bits_do_not_clobber_ordinary_object_loads() {
    let context = Context::create();
    let profile =
        ValidatedBackendProfile::for_selection(scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .unwrap()
            .with_optimization(OptimizationMode::Release);
    let machine = profile.create_target_machine().unwrap();
    let llvm = parse(
        &context,
        r#"
define i64 @f(ptr addrspace(1) %object) gc "statepoint-example" {
entry:
  %before = load i64, ptr addrspace(1) %object
  %address = ptrtoint ptr addrspace(1) %object to i64
  %after = load i64, ptr addrspace(1) %object
  %difference = sub i64 %after, %before
  %result = add i64 %address, %difference
  ret i64 %result
}
"#,
    );
    crate::statepoint::rewrite(&llvm, &machine, &manifest(None), profile).unwrap();
    llvm.verify().unwrap();
    let ir = llvm.print_to_string().to_string();
    let function = llvm.get_function("f").unwrap();
    assert!(
        function
            .get_basic_blocks()
            .iter()
            .flat_map(|block| block.get_instructions())
            .all(|instruction| instruction.get_opcode() != InstructionOpcode::Load),
        "{ir}"
    );
    assert!(
        ir.contains("ptrtoint ptr addrspace(1) %object to i64"),
        "{ir}"
    );
    assert!(!ir.contains("scoop.gc.address.bits"), "{ir}");
}
