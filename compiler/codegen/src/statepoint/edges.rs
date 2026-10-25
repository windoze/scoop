//! Split a selected normal CFG edge while preserving each successor phi value.

use inkwell::llvm_sys::core::{LLVMGetNumSuccessors, LLVMGetSuccessor, LLVMSetSuccessor};
use inkwell::values::PhiValue;

use crate::statepoint::{AsValueRef, BasicBlock, CodegenError, InstructionOpcode};

pub(super) fn split<'ctx>(
    predecessor: BasicBlock<'ctx>,
    successor: BasicBlock<'ctx>,
    name: &str,
) -> Result<BasicBlock<'ctx>, CodegenError> {
    let context = predecessor.get_context();
    let block = context.append_basic_block(predecessor.get_parent().expect("CFG function"), name);
    let builder = context.create_builder();
    builder.position_at_end(block);
    builder
        .build_unconditional_branch(successor)
        .map_err(|error| CodegenError(format!("CFG edge branch: {error}")))?;
    let branch = predecessor
        .get_terminator()
        .expect("predecessor terminates");
    // SAFETY: this is the live predecessor's branch. Only the selected edge
    // changes; successor phis below retain their original incoming values.
    unsafe {
        for index in 0..LLVMGetNumSuccessors(branch.as_value_ref()) {
            if LLVMGetSuccessor(branch.as_value_ref(), index) == successor.as_mut_ptr() {
                LLVMSetSuccessor(branch.as_value_ref(), index, block.as_mut_ptr());
            }
        }
    }
    let phis = successor
        .get_instructions()
        .take_while(|instruction| instruction.get_opcode() == InstructionOpcode::Phi)
        .collect::<Vec<_>>();
    for instruction in phis {
        let old = PhiValue::try_from(instruction).expect("successor phi");
        builder.position_before(&instruction);
        let new = builder
            .build_phi(old.as_basic_value().get_type(), "edge.incoming")
            .map_err(|error| CodegenError(format!("CFG edge phi: {error}")))?;
        for (value, incoming) in old.get_incomings() {
            new.add_incoming(&[(
                &value,
                if incoming == predecessor {
                    block
                } else {
                    incoming
                },
            )]);
        }
        instruction.replace_all_uses_with(&new.as_instruction());
        instruction.erase_from_basic_block();
    }
    Ok(block)
}
