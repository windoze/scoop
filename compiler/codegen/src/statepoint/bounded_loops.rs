//! Select only actual bounded, integer-only inner loops in Managed functions.

use inkwell::IntPredicate;
use inkwell::llvm_sys::core::{LLVMReplaceMDNodeOperandWith, LLVMValueAsMetadata};
use inkwell::values::{IntValue, PhiValue};

use super::*;

mod remarks;

pub(super) fn optimize(
    function: FunctionValue<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let context = function.get_type().get_context();
    let mut selected = false;
    for block in function.get_basic_blocks() {
        if !bounded(block) {
            continue;
        }
        let enable = context.metadata_node(&[
            context.metadata_string("llvm.loop.vectorize.enable").into(),
            context.bool_type().const_int(1, false).into(),
        ]);
        let interleave = context.metadata_node(&[
            context.metadata_string("llvm.loop.interleave.count").into(),
            context.i32_type().const_int(1, false).into(),
        ]);
        let unroll =
            context.metadata_node(&[context.metadata_string("llvm.loop.unroll.disable").into()]);
        let node = context.metadata_node(&[
            enable.into(),
            enable.into(),
            interleave.into(),
            unroll.into(),
        ]);
        // SAFETY: the live node belongs to this context. LLVM loop metadata
        // requires its first operand to refer to the loop node itself.
        unsafe {
            LLVMReplaceMDNodeOperandWith(
                node.as_value_ref(),
                0,
                LLVMValueAsMetadata(node.as_value_ref()),
            );
        }
        block
            .get_terminator()
            .expect("qualified loop has a branch")
            .set_metadata(node, context.get_kind_id("llvm.loop"))
            .map_err(|error| CodegenError(format!("bounded loop metadata: {error:?}")))?;
        selected = true;
    }
    if selected {
        let _remarks = remarks::Quiet::new(context);
        function.run_passes(
            "loop-simplify,lcssa,loop-vectorize<interleave-forced-only;vectorize-forced-only>,instcombine<no-verify-fixpoint>,adce",
            machine, PassBuilderOptions::create(),
        ).map_err(|error| CodegenError(format!("bounded inner loop optimization failed: {error}")))?;
    }
    Ok(())
}

fn bounded(block: BasicBlock<'_>) -> bool {
    let Some(branch) = block.get_terminator() else {
        return false;
    };
    if branch.get_opcode() != InstructionOpcode::Br || branch.get_num_operands() != 3 {
        return false;
    }
    let Some(condition) = integer(branch, 0).and_then(|value| value.as_instruction_value()) else {
        return false;
    };
    // LLVM's branch operands store the false successor before the true one.
    let again = match condition.get_icmp_predicate() {
        Some(IntPredicate::NE) => branch.get_operand(2).and_then(|operand| operand.block()),
        Some(IntPredicate::EQ) => branch.get_operand(1).and_then(|operand| operand.block()),
        _ => return false,
    };
    let exit = match condition.get_icmp_predicate() {
        Some(IntPredicate::NE) => branch.get_operand(1).and_then(|operand| operand.block()),
        _ => branch.get_operand(2).and_then(|operand| operand.block()),
    };
    if again != Some(block) || exit == Some(block) {
        return false;
    }
    let Some(value) = integer(condition, 0).and_then(|value| value.as_instruction_value()) else {
        return false;
    };
    let Some(compare) = integer(condition, 1).and_then(|value| value.get_zero_extended_constant())
    else {
        return false;
    };
    let phi = if compare == 0 {
        decrement(value)
            .and_then(|value| value.as_instruction_value())
            .and_then(|value| PhiValue::try_from(value).ok())
    } else if compare == 1 {
        PhiValue::try_from(value).ok()
    } else {
        None
    };
    let Some(phi) = phi else {
        return false;
    };
    if phi.as_instruction().get_parent() != Some(block) || phi.count_incoming() != 2 {
        return false;
    }
    let mut entry = false;
    let mut backedge = false;
    for (incoming, predecessor) in phi.get_incomings() {
        if predecessor == block {
            backedge = incoming
                .as_instruction_value()
                .and_then(decrement)
                .is_some_and(|counter| counter.as_basic_value_enum() == phi.as_basic_value());
            if compare == 0 && incoming.as_instruction_value() != Some(value) {
                return false;
            }
        } else {
            entry = incoming.is_int_value()
                && incoming
                    .into_int_value()
                    .get_zero_extended_constant()
                    .is_some_and(|count| (1..=64).contains(&count));
        }
    }
    entry
        && backedge
        && block.get_instructions().all(|instruction| {
            if instruction == branch {
                return true;
            }
            instruction.get_type().is_int_type()
                && instruction.get_operands().all(|operand| {
                    operand.is_some_and(|operand| {
                        operand.value().is_some_and(|value| value.is_int_value())
                    })
                })
                && matches!(
                    instruction.get_opcode(),
                    InstructionOpcode::Phi
                        | InstructionOpcode::Add
                        | InstructionOpcode::Sub
                        | InstructionOpcode::Mul
                        | InstructionOpcode::And
                        | InstructionOpcode::Or
                        | InstructionOpcode::Xor
                        | InstructionOpcode::Shl
                        | InstructionOpcode::LShr
                        | InstructionOpcode::AShr
                        | InstructionOpcode::Trunc
                        | InstructionOpcode::SExt
                        | InstructionOpcode::ZExt
                        | InstructionOpcode::ICmp
                        | InstructionOpcode::Select
                )
        })
}

fn integer(instruction: InstructionValue<'_>, index: u32) -> Option<IntValue<'_>> {
    let value = instruction.get_operand(index)?.value()?;
    value.is_int_value().then(|| value.into_int_value())
}

fn decrement(instruction: InstructionValue<'_>) -> Option<IntValue<'_>> {
    let lhs = integer(instruction, 0)?;
    let rhs = integer(instruction, 1)?;
    match instruction.get_opcode() {
        InstructionOpcode::Sub if rhs.get_zero_extended_constant() == Some(1) => Some(lhs),
        InstructionOpcode::Add if rhs.get_sign_extended_constant() == Some(-1) => Some(lhs),
        _ => None,
    }
}
