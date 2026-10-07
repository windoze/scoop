//! CFG paths bounded by dynamic SSA redefinitions, including PHI incoming edges.

use super::*;
use inkwell::basic_block::BasicBlock;
use inkwell::llvm_sys::core::{
    LLVMCountIncoming, LLVMGetIncomingBlock, LLVMGetIncomingValue, LLVMGetNumSuccessors,
    LLVMGetSuccessor,
};
use inkwell::values::FunctionValue;
use std::collections::BTreeSet;

/// Determine whether a particular static use can execute after `site` while
/// still observing the value produced by `definition`. Re-entering the
/// definition's block through a loop executes the definition again and is a
/// barrier: that dynamic value was produced after the statepoint and is safe.
pub(super) fn use_reachable_without_redefinition(
    site: InstructionValue<'_>,
    user: InstructionValue<'_>,
    definition: InstructionValue<'_>,
    function: FunctionValue<'_>,
) -> Result<bool, CodegenError> {
    if user.get_opcode() == InstructionOpcode::Phi {
        let raw = user.as_value_ref();
        let count = unsafe { LLVMCountIncoming(raw) };
        for index in 0..count {
            // SAFETY: `user` is a PHI in verified LLVM IR.
            if unsafe { LLVMGetIncomingValue(raw, index) } != definition.as_value_ref() {
                continue;
            }
            let incoming_block = unsafe { LLVMGetIncomingBlock(raw, index) } as usize;
            if path_reaches_position_without_definition(
                site,
                incoming_block,
                None,
                definition,
                function,
            )? {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    let user_block = user
        .get_parent()
        .ok_or_else(|| CodegenError("instruction use has no parent block".to_string()))?;
    path_reaches_position_without_definition(
        site,
        user_block.as_mut_ptr() as usize,
        Some(instruction_position(user_block, user)?),
        definition,
        function,
    )
}

/// Search from immediately after `site` to an instruction position, or to a
/// predecessor's outgoing edge when `target_position` is absent. The search
/// stops before re-executing `definition`.
fn path_reaches_position_without_definition(
    site: InstructionValue<'_>,
    target_block: usize,
    target_position: Option<usize>,
    definition: InstructionValue<'_>,
    function: FunctionValue<'_>,
) -> Result<bool, CodegenError> {
    let site_block = site
        .get_parent()
        .ok_or_else(|| CodegenError("statepoint has no parent block".to_string()))?;
    let definition_block = definition
        .get_parent()
        .ok_or_else(|| CodegenError("derived definition has no parent block".to_string()))?;
    let definition_block_key = definition_block.as_mut_ptr() as usize;
    let definition_position = instruction_position(definition_block, definition)?;
    let blocks = function
        .get_basic_blocks()
        .into_iter()
        .map(|block| (block.as_mut_ptr() as usize, block))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![(
        site_block.as_mut_ptr() as usize,
        instruction_position(site_block, site)? + 1,
    )];
    let mut visited = BTreeSet::new();

    while let Some((block_key, start)) = pending.pop() {
        if !visited.insert((block_key, start)) {
            continue;
        }
        let block = blocks.get(&block_key).ok_or_else(|| {
            CodegenError("LLVM CFG traversal reached a block outside its function".to_string())
        })?;
        let barrier = (block_key == definition_block_key && definition_position >= start)
            .then_some(definition_position);
        if block_key == target_block {
            match target_position {
                Some(position) if position >= start && barrier.is_none_or(|end| position < end) => {
                    return Ok(true);
                }
                None if barrier.is_none() => return Ok(true),
                Some(_) | None => {}
            }
        }
        if barrier.is_some() {
            continue;
        }
        pending.extend(
            block_successors(*block)?
                .into_iter()
                .map(|successor| (successor, 0)),
        );
    }
    Ok(false)
}

pub(super) fn instruction_can_reach(
    definition: InstructionValue<'_>,
    target: InstructionValue<'_>,
    function: FunctionValue<'_>,
) -> Result<bool, CodegenError> {
    let definition_block = definition
        .get_parent()
        .ok_or_else(|| CodegenError("instruction definition has no parent block".to_string()))?;
    let target_block = target
        .get_parent()
        .ok_or_else(|| CodegenError("target instruction has no parent block".to_string()))?;
    if definition_block == target_block
        && instruction_position(definition_block, definition)?
            < instruction_position(target_block, target)?
    {
        return Ok(true);
    }
    Ok(blocks_reachable_after(definition, function)?
        .contains(&(target_block.as_mut_ptr() as usize)))
}

pub(super) fn blocks_reachable_after(
    instruction: InstructionValue<'_>,
    function: FunctionValue<'_>,
) -> Result<BTreeSet<usize>, CodegenError> {
    let source = instruction
        .get_parent()
        .ok_or_else(|| CodegenError("statepoint has no parent block".to_string()))?;
    let blocks = function
        .get_basic_blocks()
        .into_iter()
        .map(|block| (block.as_mut_ptr() as usize, block))
        .collect::<BTreeMap<_, _>>();
    let mut pending = block_successors(source)?;
    let mut reachable = BTreeSet::new();
    while let Some(block_key) = pending.pop() {
        if !reachable.insert(block_key) {
            continue;
        }
        let block = blocks.get(&block_key).ok_or_else(|| {
            CodegenError("LLVM CFG successor belongs to another function".to_string())
        })?;
        pending.extend(block_successors(*block)?);
    }
    Ok(reachable)
}

fn block_successors(block: BasicBlock<'_>) -> Result<Vec<usize>, CodegenError> {
    let Some(terminator) = block.get_terminator() else {
        return Err(CodegenError(
            "LLVM basic block has no terminator after RS4GC".to_string(),
        ));
    };
    let raw = terminator.as_value_ref();
    // SAFETY: LLVMGetNumSuccessors/LLVMGetSuccessor accept terminators from
    // verified IR and return borrowed basic-block references.
    let count = unsafe { LLVMGetNumSuccessors(raw) };
    Ok((0..count)
        .map(|index| unsafe { LLVMGetSuccessor(raw, index) } as usize)
        .collect())
}

pub(super) fn instruction_position(
    block: BasicBlock<'_>,
    target: InstructionValue<'_>,
) -> Result<usize, CodegenError> {
    block
        .get_instructions()
        .position(|instruction| instruction == target)
        .ok_or_else(|| CodegenError("instruction is absent from its parent block".to_string()))
}

pub(super) fn phi_uses_value_after_statepoint(
    phi: InstructionValue<'_>,
    value: inkwell::llvm_sys::prelude::LLVMValueRef,
    site_block: BasicBlock<'_>,
    post_blocks: &BTreeSet<usize>,
) -> bool {
    let raw = phi.as_value_ref();
    // SAFETY: the caller only passes a PHI instruction from verified IR.
    let count = unsafe { LLVMCountIncoming(raw) };
    (0..count).any(|index| unsafe {
        LLVMGetIncomingValue(raw, index) == value
            && (LLVMGetIncomingBlock(raw, index) == site_block.as_mut_ptr()
                || post_blocks.contains(&(LLVMGetIncomingBlock(raw, index) as usize)))
    })
}
