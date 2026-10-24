//! Reuse mapping queries and combine marks in bounded, non-synchronizing loops.

use std::collections::{HashMap, HashSet};

use inkwell::targets::TargetMachine;

use super::{edges, operations, region::Region};
use crate::statepoint::{
    BasicBlock, BasicValue, CodegenError, InstructionOpcode, InstructionValue,
};

mod batches;
mod query;
mod stack_reads;

pub(super) fn prepare(
    region: &mut Region<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    // Only AArch64 showed a stable gain from moving queries past stack reloads.
    // Ordinary reference loops keep the existing target-independent selection.
    let retain_stack_reloads = machine.get_target().get_name() == c"aarch64";
    if !region
        .blocks
        .iter()
        .flat_map(|b| b.get_instructions())
        .all(|instruction| supported(instruction, retain_stack_reloads))
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
    if !plan.is_empty() {
        let preheader = preheader(region)?;
        let builder = region.header.get_context().create_builder();
        builder.position_before(&preheader.get_terminator().expect("preheader terminates"));
        for instruction in plan {
            instruction.remove_from_basic_block();
            builder.insert_instruction(&instruction, None);
        }
        region.predecessor = preheader;
    }
    batches::coalesce(region, machine, &mandatory)
}

fn supported(instruction: InstructionValue<'_>, retain_stack_reloads: bool) -> bool {
    match instruction.get_opcode() {
        InstructionOpcode::Load => {
            operations::ordinary(instruction)
                || query::page_load(instruction)
                || (retain_stack_reloads && stack_reads::retained_load(instruction))
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
    edges::split(region.predecessor, region.header, "card.preheader")
}
