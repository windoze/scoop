//! Prove every path through an iteration reaches the same bounded backedge.

use std::collections::HashMap;

use inkwell::IntPredicate;
use inkwell::values::{IntValue, PhiValue};

use super::{BasicBlock, BasicValue, InstructionOpcode, InstructionValue};

pub(super) struct Region<'ctx> {
    pub header: BasicBlock<'ctx>,
    pub latch: BasicBlock<'ctx>,
    pub predecessor: BasicBlock<'ctx>,
    pub exit: BasicBlock<'ctx>,
    pub iterations: u64,
    pub blocks: Vec<BasicBlock<'ctx>>,
}

pub(super) fn analyze(latch: BasicBlock<'_>) -> Option<Region<'_>> {
    let branch = latch.get_terminator()?;
    if branch.get_opcode() != InstructionOpcode::Br || branch.get_num_operands() != 3 {
        return None;
    }
    let condition = integer(branch, 0).and_then(|value| value.as_instruction_value())?;
    // LLVM's branch operands store the false successor before the true one.
    let (again, exit) = match condition.get_icmp_predicate() {
        Some(IntPredicate::NE) => (2, 1),
        Some(IntPredicate::EQ) => (1, 2),
        _ => return None,
    };
    let header = branch.get_operand(again)?.block()?;
    let exit = branch.get_operand(exit)?.block()?;
    if exit == header {
        return None;
    }
    let value = integer(condition, 0).and_then(|value| value.as_instruction_value())?;
    let compare = integer(condition, 1).and_then(|value| value.get_zero_extended_constant())?;
    let phi = if compare == 0 {
        decrement(value)
            .and_then(|value| value.as_instruction_value())
            .and_then(|value| PhiValue::try_from(value).ok())
    } else if compare == 1 {
        PhiValue::try_from(value).ok()
    } else {
        None
    }?;
    if phi.as_instruction().get_parent() != Some(header) || phi.count_incoming() != 2 {
        return None;
    }
    let mut entry = None;
    let mut backedge = false;
    for (incoming, predecessor) in phi.get_incomings() {
        if predecessor == latch {
            let update = incoming.as_instruction_value()?;
            backedge = update.get_parent() == Some(latch)
                && decrement(update)
                    .is_some_and(|counter| counter.as_basic_value_enum() == phi.as_basic_value());
            if compare == 0 && update != value {
                return None;
            }
        } else {
            let count = incoming
                .is_int_value()
                .then(|| incoming.into_int_value().get_zero_extended_constant())
                .flatten()?;
            entry = (1..=64).contains(&count).then_some((predecessor, count));
        }
    }
    let (predecessor, iterations) = entry.filter(|_| backedge)?;
    let blocks = acyclic(header, latch, exit)?;
    if blocks.contains(&predecessor) {
        return None;
    }
    for block in header.get_parent()?.get_basic_blocks() {
        if blocks.contains(&block) {
            continue;
        }
        let terminator = block.get_terminator()?;
        for next in terminator
            .get_operands()
            .filter_map(|operand| operand.and_then(|value| value.block()))
        {
            if blocks.contains(&next) && !(block == predecessor && next == header) {
                return None;
            }
        }
    }
    Some(Region {
        header,
        latch,
        predecessor,
        exit,
        iterations,
        blocks,
    })
}

fn acyclic<'ctx>(
    header: BasicBlock<'ctx>,
    latch: BasicBlock<'ctx>,
    exit: BasicBlock<'ctx>,
) -> Option<Vec<BasicBlock<'ctx>>> {
    let mut colors = HashMap::new();
    let mut pending = vec![(header, false)];
    let mut blocks = Vec::new();
    while let Some((block, finish)) = pending.pop() {
        if block == exit {
            return None;
        }
        if finish {
            colors.insert(block, 2);
            blocks.push(block);
            continue;
        }
        match colors.get(&block) {
            Some(1) => return None,
            Some(2) => continue,
            _ => {}
        }
        colors.insert(block, 1);
        pending.push((block, true));
        if block == latch {
            continue;
        }
        let branch = block.get_terminator()?;
        if branch.get_opcode() != InstructionOpcode::Br {
            return None;
        }
        pending.extend(
            branch
                .get_operands()
                .filter_map(|operand| operand.and_then(|value| value.block()))
                .map(|next| (next, false)),
        );
    }
    if !colors.contains_key(&latch) {
        return None;
    }
    blocks.reverse();
    Some(blocks)
}

fn integer(instruction: InstructionValue<'_>, index: u32) -> Option<IntValue<'_>> {
    let value = instruction.get_operand(index)?.value()?;
    value.is_int_value().then(|| value.into_int_value())
}

fn decrement(instruction: InstructionValue<'_>) -> Option<IntValue<'_>> {
    let opcode = instruction.get_opcode();
    // Other integer-producing instructions can have non-basic operands,
    // including the statepoint token consumed by gc.result.
    if !matches!(opcode, InstructionOpcode::Sub | InstructionOpcode::Add) {
        return None;
    }
    let lhs = integer(instruction, 0)?;
    let rhs = integer(instruction, 1)?;
    match opcode {
        InstructionOpcode::Sub if rhs.get_zero_extended_constant() == Some(1) => Some(lhs),
        InstructionOpcode::Add if rhs.get_sign_extended_constant() == Some(-1) => Some(lhs),
        _ => None,
    }
}
