use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::FileType;
use inkwell::values::{
    AnyValue, AsValueRef, BasicValue, BasicValueEnum, CallSiteValue, InstructionOpcode, PhiValue,
};
use scoop_lir::OptimizationMode;

pub(super) fn field(mut value: BasicValueEnum<'_>, index: u32) -> BasicValueEnum<'_> {
    loop {
        let instruction = value.as_instruction_value().expect("rebuilt aggregate");
        if instruction.get_opcode() == InstructionOpcode::ExtractValue {
            value = field(
                instruction.get_operand(0).unwrap().value().unwrap(),
                instruction.get_indices()[0],
            );
            continue;
        }
        assert_eq!(instruction.get_opcode(), InstructionOpcode::InsertValue);
        if instruction.get_indices() == [index] {
            return instruction.get_operand(1).unwrap().value().unwrap();
        }
        value = instruction.get_operand(0).unwrap().value().unwrap();
    }
}

#[test]
fn real_aggregate_return_store_and_argument_use_the_relocated_object() {
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
            let llvm = parse(&context, include_str!("aggregate_uses.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let expected = manifest(Some((
                7,
                ExpectedStatepoint::Relocating(
                    vec![ExpectedRoot {
                        source: CallerRootSource::Temp(la_arena::Idx::from_raw(0.into())),
                        byte_offset: 0,
                    }]
                    .into(),
                ),
            )));
            crate::statepoint::optimize(&llvm, &machine, &expected, profile).unwrap();
            let plan = crate::statepoint::lower(&llvm, &machine).unwrap();
            llvm.run_passes(
                "function(verify<safepoint-ir>),verify",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
            let block = llvm
                .get_function("f")
                .unwrap()
                .get_first_basic_block()
                .unwrap();
            let returned = block
                .get_terminator()
                .unwrap()
                .get_operand(0)
                .unwrap()
                .value()
                .unwrap();
            let object = field(returned, 0);
            assert!(
                object
                    .print_to_string()
                    .to_string()
                    .contains("@llvm.experimental.gc.relocate")
            );
            let stored = block
                .get_instructions()
                .find(|instruction| instruction.get_opcode() == InstructionOpcode::Store)
                .unwrap()
                .get_operand(0)
                .unwrap()
                .value()
                .unwrap();
            let stored_object = if stored.is_pointer_value() {
                stored
            } else {
                field(stored, 0)
            };
            assert_eq!(stored_object, object);
            let consumer = block
                .get_instructions()
                .find(|instruction| {
                    if instruction.get_opcode() != InstructionOpcode::Call {
                        return false;
                    }
                    // SAFETY: this instruction is a CallBase.
                    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
                    call.get_called_fn_value()
                        .is_some_and(|callee| callee.get_name().to_bytes() == b"consume")
                })
                .unwrap();
            assert_eq!(consumer.get_operand(0).unwrap().value().unwrap(), object);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("aggregate.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_root_counts(&path, &plan);
        }
    }
}

#[test]
fn aggregate_control_flow_rebuilds_actual_uses_from_current_scalar_components() {
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
            let llvm = parse(&context, include_str!("aggregate_control.ll"));
            llvm.set_triple(&machine.get_triple());
            llvm.set_data_layout(&machine.get_target_data().get_data_layout());
            let expected = manifest(Some((
                7,
                ExpectedStatepoint::Relocating(
                    vec![ExpectedRoot {
                        source: CallerRootSource::Temp(la_arena::Idx::from_raw(0.into())),
                        byte_offset: 0,
                    }]
                    .into(),
                ),
            )));
            let plan = crate::statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
            llvm.run_passes(
                "function(verify<safepoint-ir>),verify",
                &machine,
                PassBuilderOptions::create(),
            )
            .unwrap();
            let function = llvm.get_function("f").unwrap();
            let returned = function
                .get_basic_blocks()
                .into_iter()
                .flat_map(|block| block.get_instructions())
                .find(|instruction| instruction.get_opcode() == InstructionOpcode::Return)
                .unwrap()
                .get_operand(0)
                .unwrap()
                .value()
                .unwrap();
            assert!(has_relocation(
                field(returned, 0),
                &mut std::collections::HashSet::new()
            ));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("control.o");
            machine
                .write_to_file(&llvm, FileType::Object, &path)
                .unwrap();
            assert_machine_root_counts(&path, &plan);
        }
    }
}

pub(super) fn has_relocation(
    value: BasicValueEnum<'_>,
    seen: &mut std::collections::HashSet<usize>,
) -> bool {
    if !seen.insert(value.as_value_ref() as usize) {
        return false;
    }
    let Some(instruction) = value.as_instruction_value() else {
        return false;
    };
    match instruction.get_opcode() {
        InstructionOpcode::Call => unsafe { CallSiteValue::new(instruction.as_value_ref()) }
            .get_called_fn_value()
            .is_some_and(|callee| {
                callee
                    .get_name()
                    .to_bytes()
                    .starts_with(b"llvm.experimental.gc.relocate")
            }),
        InstructionOpcode::Phi => PhiValue::try_from(instruction)
            .unwrap()
            .get_incomings()
            .any(|(value, _)| has_relocation(value, seen)),
        InstructionOpcode::Select => [1, 2].into_iter().any(|index| {
            has_relocation(
                instruction.get_operand(index).unwrap().value().unwrap(),
                seen,
            )
        }),
        _ => false,
    }
}
