//! Pointer-provenance and control-flow checks for rewritten statepoints.

use std::collections::{BTreeMap, BTreeSet};

use inkwell::basic_block::BasicBlock;
use inkwell::llvm_sys::LLVMTypeKind;
use inkwell::llvm_sys::core::{
    LLVMCountIncoming, LLVMGetIncomingBlock, LLVMGetIncomingValue, LLVMGetNumOperands,
    LLVMGetNumSuccessors, LLVMGetOperand, LLVMGetPointerAddressSpace, LLVMGetSuccessor,
    LLVMGetTypeKind, LLVMTypeOf,
};
use inkwell::values::{
    AsValueRef, BasicMetadataValueEnum, CallSiteValue, FunctionValue, InstructionOpcode,
    InstructionValue,
};

use super::{CodegenError, ObservedStatepoint, TypedManagedPointerBoundary};
use crate::target::ManagedAddressSpace;

pub(super) fn verify_pointer_instruction(
    instruction: InstructionValue<'_>,
    typed_cast_metadata: u32,
    managed_address_space: ManagedAddressSpace,
    function: &str,
) -> Result<(), CodegenError> {
    let raw = instruction.as_value_ref();
    let witness = typed_pointer_boundary(instruction, typed_cast_metadata)?;
    match instruction.get_opcode() {
        InstructionOpcode::PtrToInt => {
            // SAFETY: ptrtoint has one pointer operand in verified LLVM IR.
            if is_managed_pointer(unsafe { LLVMGetOperand(raw, 0) }, managed_address_space) {
                require_pointer_boundary(
                    witness,
                    TypedManagedPointerBoundary::CardAddress,
                    function,
                    instruction,
                )?;
            }
        }
        InstructionOpcode::IntToPtr => {
            if is_managed_pointer(raw, managed_address_space) {
                require_pointer_boundary(
                    witness,
                    TypedManagedPointerBoundary::AllocationResult,
                    function,
                    instruction,
                )?;
            }
        }
        InstructionOpcode::AddrSpaceCast => {
            // SAFETY: addrspacecast has one pointer operand in verified LLVM IR.
            let source = unsafe { LLVMGetOperand(raw, 0) };
            if is_managed_pointer(source, managed_address_space)
                || is_managed_pointer(raw, managed_address_space)
            {
                return Err(CodegenError(format!(
                    "managed address-space cast is not a supported typed boundary in `{function}`: {instruction}"
                )));
            }
        }
        _ if witness.is_some() => {
            return Err(CodegenError(format!(
                "typed managed pointer boundary witness is attached to an invalid instruction in `{function}`: {instruction}"
            )));
        }
        _ => {}
    }
    Ok(())
}

fn typed_pointer_boundary(
    instruction: InstructionValue<'_>,
    metadata_kind: u32,
) -> Result<Option<TypedManagedPointerBoundary>, CodegenError> {
    let Some(metadata) = instruction.get_metadata(metadata_kind) else {
        return Ok(None);
    };
    let values = metadata.get_node_values().ok_or_else(|| {
        CodegenError("typed managed pointer boundary metadata is not a node".to_string())
    })?;
    let [BasicMetadataValueEnum::MetadataValue(value)] = values.as_slice() else {
        return Err(CodegenError(
            "typed managed pointer boundary metadata must contain one string".to_string(),
        ));
    };
    let name = value.get_string_value().ok_or_else(|| {
        CodegenError("typed managed pointer boundary metadata operand is not a string".to_string())
    })?;
    match name {
        b"allocation-result" => Ok(Some(TypedManagedPointerBoundary::AllocationResult)),
        b"card-address" => Ok(Some(TypedManagedPointerBoundary::CardAddress)),
        _ => Err(CodegenError(format!(
            "unknown typed managed pointer boundary `{}`",
            String::from_utf8_lossy(name)
        ))),
    }
}

fn require_pointer_boundary(
    actual: Option<TypedManagedPointerBoundary>,
    expected: TypedManagedPointerBoundary,
    function: &str,
    instruction: InstructionValue<'_>,
) -> Result<(), CodegenError> {
    if actual != Some(expected) {
        return Err(CodegenError(format!(
            "managed pointer conversion in `{function}` requires typed `{}` boundary, observed {}: {instruction}",
            expected.name(),
            actual.map_or("none", TypedManagedPointerBoundary::name)
        )));
    }
    Ok(())
}

pub(super) fn is_managed_pointer(
    value: inkwell::llvm_sys::prelude::LLVMValueRef,
    managed_address_space: ManagedAddressSpace,
) -> bool {
    if value.is_null() {
        return false;
    }
    // SAFETY: LLVM values always have a type. Pointer address-space access is
    // only valid after checking the type kind.
    let ty = unsafe { LLVMTypeOf(value) };
    unsafe {
        LLVMGetTypeKind(ty) == LLVMTypeKind::LLVMPointerTypeKind
            && LLVMGetPointerAddressSpace(ty) == managed_address_space.llvm()
    }
}

pub(super) fn verify_no_stale_root_uses(
    id: u64,
    site: &ObservedStatepoint<'_>,
) -> Result<(), CodegenError> {
    let post_blocks = blocks_reachable_after(site.instruction, site.function)?;
    let site_block = site
        .instruction
        .get_parent()
        .ok_or_else(|| CodegenError(format!("statepoint {id} has no parent block")))?;
    let site_position = instruction_position(site_block, site.instruction)?;
    for (root_index, root) in site.roots.iter().enumerate() {
        for block in site.function.get_basic_blocks() {
            let block_key = block.as_mut_ptr() as usize;
            for (position, instruction) in block.get_instructions().enumerate() {
                if instruction == site.instruction {
                    continue;
                }
                let same_block_after = block == site_block && position > site_position;
                if !same_block_after && !post_blocks.contains(&block_key) {
                    continue;
                }
                let stale = if instruction.get_opcode() == InstructionOpcode::Phi {
                    phi_uses_value_after_statepoint(
                        instruction,
                        root.as_value_ref(),
                        site_block,
                        &post_blocks,
                    )
                } else {
                    instruction_uses_value(instruction, root.as_value_ref())?
                };
                if stale {
                    return Err(CodegenError(format!(
                        "statepoint {id} has a stale post-site use of gc-live root {root_index}: {instruction}"
                    )));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn verify_no_derived_live_through(
    sites: &BTreeMap<u64, ObservedStatepoint<'_>>,
    managed_address_space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    for (id, site) in sites {
        let function_name = site.function.get_name().to_string_lossy();
        for block in site.function.get_basic_blocks() {
            for derived in block.get_instructions().filter(|instruction| {
                instruction.get_opcode() == InstructionOpcode::GetElementPtr
                    && is_managed_pointer(instruction.as_value_ref(), managed_address_space)
            }) {
                if !instruction_can_reach(derived, site.instruction, site.function)? {
                    continue;
                }
                if instruction_uses_value(site.instruction, derived.as_value_ref())? {
                    return Err(CodegenError(format!(
                        "derived AS{} address crosses statepoint {id} in `{function_name}` as a call operand; definition: {derived}",
                        managed_address_space.llvm()
                    )));
                }
                for use_block in site.function.get_basic_blocks() {
                    for user in use_block.get_instructions() {
                        if user == site.instruction {
                            continue;
                        }
                        if !instruction_uses_value(user, derived.as_value_ref())? {
                            continue;
                        }
                        if use_reachable_without_redefinition(
                            site.instruction,
                            user,
                            derived,
                            site.function,
                        )? {
                            return Err(CodegenError(format!(
                                "derived AS{} address crosses statepoint {id} in `{function_name}`; definition: {derived}; post-site use: {user}",
                                managed_address_space.llvm()
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Determine whether a particular static use can execute after `site` while
/// still observing the value produced by `definition`. Re-entering the
/// definition's block through a loop executes the definition again and is a
/// barrier: that dynamic value was produced after the statepoint and is safe.
fn use_reachable_without_redefinition(
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

fn instruction_can_reach(
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

fn blocks_reachable_after(
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

fn instruction_position(
    block: BasicBlock<'_>,
    target: InstructionValue<'_>,
) -> Result<usize, CodegenError> {
    block
        .get_instructions()
        .position(|instruction| instruction == target)
        .ok_or_else(|| CodegenError("instruction is absent from its parent block".to_string()))
}

fn instruction_uses_value(
    instruction: InstructionValue<'_>,
    value: inkwell::llvm_sys::prelude::LLVMValueRef,
) -> Result<bool, CodegenError> {
    let raw = instruction.as_value_ref();
    // SAFETY: operand access is valid for every LLVM instruction.
    let operand_count = unsafe { LLVMGetNumOperands(raw) };
    if operand_count < 0 {
        return Err(CodegenError(
            "LLVM reported a negative instruction operand count".to_string(),
        ));
    }
    let operand_count = u32::try_from(operand_count)
        .map_err(|_| CodegenError("LLVM instruction operand count exceeds u32::MAX".to_string()))?;
    for index in 0..operand_count {
        if unsafe { LLVMGetOperand(raw, index) } == value {
            return Ok(true);
        }
    }
    if matches!(
        instruction.get_opcode(),
        InstructionOpcode::Call | InstructionOpcode::Invoke | InstructionOpcode::CallBr
    ) {
        // SAFETY: the opcode check proves this is a CallBase instruction.
        let call = unsafe { CallSiteValue::new(raw) };
        for bundle in call.get_operand_bundles() {
            if bundle
                .get_args()
                .any(|argument| argument.as_value_ref() == value)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn phi_uses_value_after_statepoint(
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
