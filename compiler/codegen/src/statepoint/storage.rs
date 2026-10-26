//! Let ordinary scalar optimization promote unexposed compiler root storage.

use std::collections::{HashMap, HashSet};

use inkwell::llvm_sys::core::{
    LLVMGetFirstUse, LLVMGetIntrinsicID, LLVMGetNextUse, LLVMGetUser, LLVMIsAInstruction,
};

use super::*;

const ACCESS: &str = "scoop.gc-storage-access";

pub(crate) fn mark_access(
    context: &Context,
    instruction: InstructionValue<'_>,
) -> Result<(), CodegenError> {
    instruction
        .set_metadata(context.metadata_node(&[]), context.get_kind_id(ACCESS))
        .map_err(|error| CodegenError(format!("mark compiler GC storage access: {error}")))
}

pub(super) fn promote(function: FunctionValue<'_>) -> Result<(), CodegenError> {
    let entry = function.get_first_basic_block().expect("defined function");
    let kind = entry.get_context().get_kind_id(ACCESS);
    let mut eligible = HashMap::new();
    for block in function.get_basic_blocks() {
        for instruction in block.get_instructions() {
            if instruction.get_metadata(kind).is_none() {
                continue;
            }
            let pointer = match instruction.get_opcode() {
                InstructionOpcode::Load => instruction.get_operand(0),
                InstructionOpcode::Store => instruction.get_operand(1),
                _ => continue,
            }
            .expect("memory address")
            .value()
            .unwrap();
            let Some(slot) = entry_slot(pointer, entry) else {
                continue;
            };
            if *eligible.entry(slot).or_insert_with(|| unexposed(slot)) {
                instruction.set_volatile(false).map_err(|error| {
                    CodegenError(format!("promote compiler root storage: {error}"))
                })?;
            }
        }
    }
    Ok(())
}

fn entry_slot<'ctx>(
    mut pointer: BasicValueEnum<'ctx>,
    entry: BasicBlock<'ctx>,
) -> Option<InstructionValue<'ctx>> {
    loop {
        let instruction = pointer.as_instruction_value()?;
        match instruction.get_opcode() {
            InstructionOpcode::Alloca => {
                return (instruction.get_parent() == Some(entry)).then_some(instruction);
            }
            InstructionOpcode::GetElementPtr | InstructionOpcode::BitCast => {
                pointer = instruction.get_operand(0)?.value()?;
            }
            _ => return None,
        }
    }
}

fn unexposed(slot: InstructionValue<'_>) -> bool {
    let mut pending = vec![slot];
    let mut seen = HashSet::new();
    while let Some(address) = pending.pop() {
        if !seen.insert(address) {
            continue;
        }
        // SAFETY: use chains belong to live instructions and are not mutated here.
        let mut usage = unsafe { LLVMGetFirstUse(address.as_value_ref()) };
        while !usage.is_null() {
            let raw = unsafe { LLVMGetUser(usage) };
            if unsafe { LLVMIsAInstruction(raw) }.is_null() {
                return false;
            }
            let user = unsafe { InstructionValue::new(raw) };
            match user.get_opcode() {
                InstructionOpcode::Load => {}
                InstructionOpcode::Store => {
                    if unsafe { LLVMGetOperand(raw, 0) } == address.as_value_ref() {
                        return false;
                    }
                }
                InstructionOpcode::GetElementPtr | InstructionOpcode::BitCast => pending.push(user),
                InstructionOpcode::Call if local_memory_intrinsic(user) => {}
                _ => return false,
            }
            usage = unsafe { LLVMGetNextUse(usage) };
        }
    }
    true
}

fn local_memory_intrinsic(instruction: InstructionValue<'_>) -> bool {
    // SAFETY: the caller has established a CallBase.
    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
    let Some(callee) = call.get_called_fn_value() else {
        return false;
    };
    if unsafe { LLVMGetIntrinsicID(callee.as_value_ref()) } == 0 {
        return false;
    }
    let name = callee.get_name().to_bytes();
    [
        b"llvm.memcpy.".as_slice(),
        b"llvm.memmove.",
        b"llvm.memset.",
        b"llvm.lifetime.start",
        b"llvm.lifetime.end",
    ]
    .into_iter()
    .any(|prefix| name.starts_with(prefix))
}
