//! Recognize selections whose possible pointer values are all LLVM constants.

use super::*;
use inkwell::llvm_sys::core::{
    LLVMCountIncoming, LLVMGetIncomingValue, LLVMIsAPHINode, LLVMIsASelectInst, LLVMIsConstant,
};

pub(super) fn is_constant_root(value: PointerValue<'_>) -> bool {
    let mut pending = vec![value.as_value_ref()];
    let mut visited = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value as usize) {
            continue;
        }
        // SAFETY: every work item is a live value from the verified LLVM module.
        unsafe {
            if LLVMIsConstant(value) != 0 {
                continue;
            }
            if !LLVMIsAPHINode(value).is_null() {
                for index in 0..LLVMCountIncoming(value) {
                    pending.push(LLVMGetIncomingValue(value, index));
                }
            } else if !LLVMIsASelectInst(value).is_null() {
                pending.push(LLVMGetOperand(value, 1));
                pending.push(LLVMGetOperand(value, 2));
            } else {
                return false;
            }
        }
    }
    true
}

/// LIR roots are object starts, including a merge of immortal and movable
/// objects. RS4GC otherwise assigns a null base to the immortal branch.
pub(super) fn mark_base_merge(value: PointerValue<'_>) -> Result<(), CodegenError> {
    let mut pending = vec![value];
    let mut visited = BTreeSet::new();
    let mut merges = Vec::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value.as_value_ref() as usize) {
            continue;
        }
        let Some(instruction) = value.as_instruction_value() else {
            continue;
        };
        let raw = instruction.as_value_ref();
        // SAFETY: phi incoming values and the selected/cast pointer operands
        // have the same scalar pointer type as this verified LLVM instruction.
        unsafe {
            match instruction.get_opcode() {
                InstructionOpcode::Phi => {
                    for index in 0..LLVMCountIncoming(raw) {
                        pending.push(
                            BasicValueEnum::new(LLVMGetIncomingValue(raw, index))
                                .into_pointer_value(),
                        );
                    }
                    if !is_constant_root(value) {
                        merges.push(instruction);
                    }
                }
                InstructionOpcode::Select => {
                    for index in [1, 2] {
                        pending.push(
                            BasicValueEnum::new(LLVMGetOperand(raw, index)).into_pointer_value(),
                        );
                    }
                    if !is_constant_root(value) {
                        merges.push(instruction);
                    }
                }
                InstructionOpcode::BitCast => {
                    pending.push(BasicValueEnum::new(LLVMGetOperand(raw, 0)).into_pointer_value())
                }
                InstructionOpcode::GetElementPtr => {
                    return Err(CodegenError(
                        "a final managed root merge contains a derived address".into(),
                    ));
                }
                _ => {}
            }
        }
    }
    for instruction in merges {
        let context = value.get_type().get_context();
        instruction
            .set_metadata(
                context.metadata_node(&[]),
                context.get_kind_id("is_base_value"),
            )
            .map_err(|error| {
                CodegenError(format!("cannot preserve managed merge base: {error}"))
            })?;
    }
    Ok(())
}
