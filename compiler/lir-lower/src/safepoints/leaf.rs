//! Bounded leaves return to an active caller without introducing a safepoint.

use super::*;

pub(super) fn eligible(
    context: &LoweringContext,
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<bool> {
    if function.gc_effect == lir::GcEffect::NoGc || !bounded_path(function) {
        return Ok(false);
    }
    let types = function
        .signature
        .arguments()
        .iter()
        .map(|argument| argument.logical_storage_type())
        .chain(function.signature.result().logical_storage_type())
        .chain(function.locals.values().map(|local| local.ty()))
        .chain(function.temps.values().map(|temp| &temp.ty));
    for ty in types {
        if lir_size_align(context, ty, structs, enums)?.0 > 64 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn bounded_path(function: &lir::Function) -> bool {
    let mut visited = vec![false; function.blocks.len()];
    let mut current = function.entry;
    let mut instructions = 0usize;
    loop {
        if std::mem::replace(&mut visited[arena_index(current)], true) {
            return false;
        }
        let block = &function.blocks[current];
        instructions = instructions
            .saturating_add(block.instructions.len())
            .saturating_add(1);
        if instructions > 64 || !block.instructions.iter().all(bounded_instruction) {
            return false;
        }
        match block.terminator {
            lir::Terminator::Return { .. } => return true,
            lir::Terminator::Br(next) => current = next,
            _ => return false,
        }
    }
}

fn bounded_instruction(instruction: &lir::Instruction) -> bool {
    use lir::Instruction as I;
    matches!(
        instruction,
        I::BinOp { .. }
            | I::UnaryOp { .. }
            | I::IntegerUnary { .. }
            | I::IntegerBinary { .. }
            | I::SafeIntegerDivRem { .. }
            | I::IntegerCompare { .. }
            | I::IntegerCompareTo { .. }
            | I::IntegerShift { .. }
            | I::IntegerConvert { .. }
            | I::FloatUnary { .. }
            | I::FloatBinary { .. }
            | I::FloatConversion { .. }
            | I::MakeZstValue { .. }
            | I::MakeAggregate { .. }
            | I::ExtractValue { .. }
            | I::HeapLoad { .. }
            | I::Store { .. }
    )
}
