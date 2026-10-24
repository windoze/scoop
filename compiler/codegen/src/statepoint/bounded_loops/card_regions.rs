//! Hoist immutable mapping queries only across a bounded, non-synchronizing loop.

use std::collections::{HashMap, HashSet};

use inkwell::llvm_sys::core::{LLVMGetNumSuccessors, LLVMGetSuccessor, LLVMSetSuccessor};
use inkwell::values::PhiValue;

use super::{operations, region::Region};
use crate::statepoint::{
    AsValueRef, BasicBlock, BasicValue, CodegenError, InstructionOpcode, InstructionValue,
};

mod query;

pub(super) fn prepare(region: &mut Region<'_>) -> Result<(), CodegenError> {
    if !region
        .blocks
        .iter()
        .flat_map(|b| b.get_instructions())
        .all(supported)
    {
        return Ok(());
    }
    let mandatory = mandatory_blocks(region);
    let mut plan = Vec::new();
    for instruction in region.blocks.iter().flat_map(|b| b.get_instructions()) {
        if !query::immutable_load(instruction) || plan.contains(&instruction) {
            continue;
        }
        let mut pending = Vec::new();
        if collect(instruction, region, &mandatory, &plan, &mut pending) {
            plan.extend(pending);
        }
    }
    if plan.is_empty() {
        return Ok(());
    }
    let preheader = preheader(region)?;
    let builder = region.header.get_context().create_builder();
    builder.position_before(&preheader.get_terminator().expect("preheader terminates"));
    for instruction in plan {
        instruction.remove_from_basic_block();
        builder.insert_instruction(&instruction, None);
    }
    region.predecessor = preheader;
    Ok(())
}

fn supported(instruction: InstructionValue<'_>) -> bool {
    match instruction.get_opcode() {
        InstructionOpcode::Load => {
            operations::ordinary(instruction) || query::page_load(instruction)
        }
        InstructionOpcode::Store => {
            operations::ordinary(instruction) || query::card_store(instruction)
        }
        InstructionOpcode::Br
        | InstructionOpcode::Phi
        | InstructionOpcode::ExtractValue
        | InstructionOpcode::InsertValue
        | InstructionOpcode::FAdd
        | InstructionOpcode::FSub
        | InstructionOpcode::FMul
        | InstructionOpcode::FDiv
        | InstructionOpcode::FRem
        | InstructionOpcode::FNeg
        | InstructionOpcode::FCmp
        | InstructionOpcode::SIToFP
        | InstructionOpcode::UIToFP
        | InstructionOpcode::FPTrunc
        | InstructionOpcode::FPExt => true,
        _ => pure_address(instruction),
    }
}

fn pure_address(instruction: InstructionValue<'_>) -> bool {
    matches!(
        instruction.get_opcode(),
        InstructionOpcode::GetElementPtr
            | InstructionOpcode::PtrToInt
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
            | InstructionOpcode::BitCast
            | InstructionOpcode::Select
    )
}

fn collect<'ctx>(
    instruction: InstructionValue<'ctx>,
    region: &Region<'ctx>,
    mandatory: &HashSet<BasicBlock<'ctx>>,
    plan: &[InstructionValue<'ctx>],
    pending: &mut Vec<InstructionValue<'ctx>>,
) -> bool {
    if plan.contains(&instruction)
        || pending.contains(&instruction)
        || !instruction
            .get_parent()
            .is_some_and(|b| region.blocks.contains(&b))
    {
        return true;
    }
    if !mandatory.contains(
        &instruction
            .get_parent()
            .expect("loop instruction has a block"),
    ) || !(pure_address(instruction)
        || query::immutable_load(instruction)
        || operations::root_load(instruction, &region.blocks))
    {
        return false;
    }
    for operand in instruction.get_operands() {
        let Some(value) = operand.and_then(|o| o.value()) else {
            return false;
        };
        if value
            .as_instruction_value()
            .is_some_and(|definition| !collect(definition, region, mandatory, plan, pending))
        {
            return false;
        }
    }
    pending.push(instruction);
    true
}

fn mandatory_blocks<'ctx>(region: &Region<'ctx>) -> HashSet<BasicBlock<'ctx>> {
    let mut dominators: HashMap<_, HashSet<_>> = HashMap::new();
    for &block in &region.blocks {
        let mut incoming = region.blocks.iter().copied().filter(|&predecessor| {
            predecessor != region.latch
                && predecessor.get_terminator().is_some_and(|branch| {
                    branch
                        .get_operands()
                        .any(|o| o.and_then(|v| v.block()) == Some(block))
                })
        });
        let mut common = incoming
            .next()
            .map(|p| dominators[&p].clone())
            .unwrap_or_default();
        for predecessor in incoming {
            common.retain(|b| dominators[&predecessor].contains(b));
        }
        common.insert(block);
        dominators.insert(block, common);
    }
    dominators
        .remove(&region.latch)
        .expect("region contains its latch")
}

fn preheader<'ctx>(region: &Region<'ctx>) -> Result<BasicBlock<'ctx>, CodegenError> {
    let branch = region
        .predecessor
        .get_terminator()
        .expect("predecessor terminates");
    if branch.get_num_operands() == 1 {
        return Ok(region.predecessor);
    }
    let context = region.header.get_context();
    let block = context.append_basic_block(
        region.header.get_parent().expect("loop function"),
        "card.preheader",
    );
    let builder = context.create_builder();
    builder.position_at_end(block);
    builder
        .build_unconditional_branch(region.header)
        .map_err(|error| CodegenError(format!("card preheader branch: {error}")))?;
    // SAFETY: this is the live predecessor's branch; only its edge to this
    // header changes. The header phis below receive the same incoming values.
    unsafe {
        for index in 0..LLVMGetNumSuccessors(branch.as_value_ref()) {
            if LLVMGetSuccessor(branch.as_value_ref(), index) == region.header.as_mut_ptr() {
                LLVMSetSuccessor(branch.as_value_ref(), index, block.as_mut_ptr());
            }
        }
    }
    let phis = region
        .header
        .get_instructions()
        .take_while(|i| i.get_opcode() == InstructionOpcode::Phi)
        .collect::<Vec<_>>();
    for instruction in phis {
        let old = PhiValue::try_from(instruction).expect("header phi");
        builder.position_before(&instruction);
        let new = builder
            .build_phi(old.as_basic_value().get_type(), "card.incoming")
            .map_err(|error| CodegenError(format!("card preheader phi: {error}")))?;
        for (value, predecessor) in old.get_incomings() {
            new.add_incoming(&[(
                &value,
                if predecessor == region.predecessor {
                    block
                } else {
                    predecessor
                },
            )]);
        }
        instruction.replace_all_uses_with(&new.as_instruction());
        instruction.erase_from_basic_block();
    }
    Ok(block)
}
