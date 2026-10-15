//! Pointer-provenance and control-flow checks for rewritten statepoints.

use std::collections::{BTreeMap, HashSet};

use inkwell::llvm_sys::LLVMTypeKind;
use inkwell::llvm_sys::core::{
    LLVMGetOperand, LLVMGetPointerAddressSpace, LLVMGetTypeKind, LLVMGetUser, LLVMIsAInstruction,
    LLVMTypeOf,
};
use inkwell::values::{
    AsValueRef, BasicMetadataValueEnum, BasicValue, BasicValueUse, FunctionValue,
    InstructionOpcode, InstructionValue,
};

use super::{CodegenError, ObservedStatepoint, TypedManagedPointerBoundary};
use crate::target::ManagedAddressSpace;

mod flow;
use flow::{FunctionFlow, phi_uses_value_after_statepoint};

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
            let source_managed = is_managed_pointer(source, managed_address_space);
            // SAFETY: an addrspacecast always has a pointer result.
            let target_space = unsafe { LLVMGetPointerAddressSpace(LLVMTypeOf(raw)) };
            if source_managed && target_space == 0 {
                require_pointer_boundary(
                    witness,
                    TypedManagedPointerBoundary::ScopedDataBorrow,
                    function,
                    instruction,
                )?;
            } else if source_managed
                || is_managed_pointer(raw, managed_address_space)
                || witness.is_some()
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
        b"scoped-data-borrow" => Ok(Some(TypedManagedPointerBoundary::ScopedDataBorrow)),
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

fn verify_no_stale_root_uses(
    id: u64,
    site: &ObservedStatepoint<'_>,
    flow: &FunctionFlow,
) -> Result<(), CodegenError> {
    if site.roots.is_empty() {
        return Ok(());
    }
    let post_blocks = flow.blocks_reachable_after(site.instruction)?;
    let (site_block, site_position) = flow.position(site.instruction)?;
    for (root_index, root) in site.roots.iter().enumerate() {
        for instruction in instruction_users(root.get_first_use(), site.function) {
            if instruction == site.instruction {
                continue;
            }
            let (block, position) = flow.position(instruction)?;
            if !(block == site_block && position > site_position) && !post_blocks.contains(&block) {
                continue;
            }
            let stale = if let Some(definition) = root.as_instruction_value() {
                flow.use_reachable_without_redefinition(site.instruction, instruction, definition)?
            } else if instruction.get_opcode() == InstructionOpcode::Phi {
                phi_uses_value_after_statepoint(
                    instruction,
                    root.as_value_ref(),
                    site_block,
                    &post_blocks,
                )
            } else {
                true
            };
            if stale {
                return Err(CodegenError(format!(
                    "statepoint {id} has a stale post-site use of gc-live root {root_index}: {instruction}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn verify_lifetimes(
    sites: &BTreeMap<u64, ObservedStatepoint<'_>>,
    managed_address_space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    let mut functions = BTreeMap::<_, Vec<_>>::new();
    for (id, site) in sites {
        functions
            .entry(site.function.as_value_ref() as usize)
            .or_default()
            .push((*id, site));
    }
    for sites in functions.values() {
        let function = sites[0].1.function;
        let function_name = function.get_name().to_string_lossy();
        let flow = FunctionFlow::new(function)?;
        for block in function.get_basic_blocks() {
            for derived in block.get_instructions().filter(|instruction| {
                instruction.get_opcode() == InstructionOpcode::GetElementPtr
                    && is_managed_pointer(instruction.as_value_ref(), managed_address_space)
            }) {
                let users = instruction_users(derived.get_first_use(), function);
                for (id, site) in sites {
                    for &user in &users {
                        if user == site.instruction {
                            if flow.instruction_can_reach(derived, site.instruction)? {
                                return Err(CodegenError(format!(
                                    "derived AS{} address crosses statepoint {id} in `{function_name}` as a call operand; definition: {derived}",
                                    managed_address_space.llvm()
                                )));
                            }
                            continue;
                        }
                        if flow.use_reachable_without_redefinition(
                            site.instruction,
                            user,
                            derived,
                        )? && flow.instruction_can_reach(derived, site.instruction)?
                        {
                            return Err(CodegenError(format!(
                                "derived AS{} address crosses statepoint {id} in `{function_name}`; definition: {derived}; post-site use: {user}",
                                managed_address_space.llvm()
                            )));
                        }
                    }
                }
            }
        }
        for (id, site) in sites {
            verify_no_stale_root_uses(*id, site, &flow)?;
        }
    }
    Ok(())
}

fn instruction_users<'ctx>(
    first_use: Option<BasicValueUse<'ctx>>,
    function: FunctionValue<'ctx>,
) -> Vec<InstructionValue<'ctx>> {
    let mut seen = HashSet::new();
    std::iter::successors(first_use, |used| used.get_next_use())
        .filter_map(|used| {
            // SAFETY: the use belongs to a live LLVM value. Inspect its user
            // directly: statepoint tokens are not Inkwell AnyValueEnum values.
            let raw = unsafe { LLVMGetUser(used.as_mut_ptr()) };
            if unsafe { LLVMIsAInstruction(raw) }.is_null() {
                return None;
            }
            let user = unsafe { InstructionValue::new(raw) };
            (user.get_parent().and_then(|block| block.get_parent()) == Some(function)
                && seen.insert(raw as usize))
            .then_some(user)
        })
        .collect()
}
