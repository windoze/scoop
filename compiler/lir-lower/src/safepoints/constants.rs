use std::collections::HashMap;

use super::*;

mod integers;
#[cfg(test)]
mod tests;
mod values;

use values::{boolean_value, known_result, known_value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KnownValue {
    Scalar(lir::Value),
    Variant(lir::LirVariantRef),
}

/// Propagate already evaluated constants along straight CFG chains before
/// assigning roots and sites. Joins and backedges begin with unknown facts.
pub(super) fn fold_constant_branches(function: &mut lir::Function) {
    if !function
        .blocks
        .values()
        .any(|block| matches!(block.terminator, lir::Terminator::CondBr { .. }))
    {
        return;
    }
    let mut address_taken = HashSet::new();
    for instruction in function
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
    {
        if let lir::Instruction::LocalAddress { local, .. } = instruction {
            address_taken.insert(*local);
        }
        for value in instruction_uses(instruction, function) {
            if let lir::Value::CArgumentStorage(storage) = value {
                address_taken.insert(storage.local());
            }
        }
    }
    let mut incoming = vec![0usize; function.blocks.len()];
    incoming[arena_index(function.entry)] = 1;
    for block in function.blocks.values() {
        for successor in block_successors(block) {
            incoming[arena_index(successor)] += 1;
        }
    }
    let starts = std::iter::once(function.entry)
        .chain(
            function
                .blocks
                .iter()
                .filter_map(|(id, _)| (incoming[arena_index(id)] != 1).then_some(id)),
        )
        .chain(function.blocks.iter().map(|(id, _)| id))
        .collect::<Vec<_>>();
    let mut visited = vec![false; function.blocks.len()];
    for mut id in starts {
        let mut constants = BlockConstants::default();
        while !visited[arena_index(id)] {
            visited[arena_index(id)] = true;
            let block = &mut function.blocks[id];
            for instruction in &block.instructions {
                constants.apply(instruction, &function.locals, &address_taken);
            }
            if let lir::Terminator::CondBr {
                cond,
                then_block,
                else_block,
            } = block.terminator
                && let Some(value) = boolean_value(cond, &constants.values)
            {
                block.terminator = lir::Terminator::Br(if value { then_block } else { else_block });
            }
            let successors = block_successors(block);
            let [next] = successors.as_slice() else {
                break;
            };
            if incoming[arena_index(*next)] != 1 {
                break;
            }
            id = *next;
        }
    }
}

#[derive(Default)]
struct BlockConstants {
    values: HashMap<LiveValue, KnownValue>,
    addresses: HashMap<LiveValue, lir::LocalId>,
}

impl BlockConstants {
    fn address(&self, value: lir::Value) -> Option<lir::LocalId> {
        self.addresses.get(&LiveValue::from_value(value)?).copied()
    }

    fn forget(&mut self, value: LiveValue) {
        self.values.remove(&value);
        self.addresses.remove(&value);
    }

    fn apply(
        &mut self,
        instruction: &lir::Instruction,
        locals: &Arena<lir::Local>,
        address_taken: &HashSet<lir::LocalId>,
    ) {
        let mut result = known_result(instruction, &self.values);
        let mut address = match *instruction {
            lir::Instruction::LocalAddress { out, local } => Some((LiveValue::Temp(out), local)),
            lir::Instruction::Store { local, value } => self
                .address(value)
                .map(|target| (LiveValue::Local(local), target)),
            lir::Instruction::ULongToPtr { out, value }
            | lir::Instruction::PtrToULong { out, value } => self
                .address(value)
                .map(|target| (LiveValue::Temp(out), target)),
            _ => None,
        };
        let mut clobber_addresses = instruction.safepoint().is_some()
            || matches!(
                instruction,
                lir::Instruction::Call { .. } | lir::Instruction::Invoke { .. }
            );
        match instruction {
            lir::Instruction::RawStore {
                pointer,
                value,
                pointee,
            } => {
                if let Some(local) = self
                    .address(*pointer)
                    .filter(|local| locals[*local].ty() == pointee.storage_type())
                {
                    let destination = LiveValue::Local(local);
                    result = known_value(*value, &self.values)
                        .filter(|value| matches!(value, KnownValue::Scalar(_)))
                        .map(|value| (destination, value));
                    address = self.address(*value).map(|target| (destination, target));
                    self.forget(destination);
                } else {
                    clobber_addresses = true;
                }
            }
            lir::Instruction::RawLoad {
                out,
                pointer,
                pointee,
            } => {
                if let Some(local) = self
                    .address(*pointer)
                    .filter(|local| locals[*local].ty() == pointee.storage_type())
                {
                    let source = lir::Value::Local(local);
                    let destination = LiveValue::Temp(*out);
                    result = known_value(source, &self.values)
                        .filter(|value| matches!(value, KnownValue::Scalar(_)))
                        .map(|value| (destination, value));
                    address = self.address(source).map(|target| (destination, target));
                }
            }
            _ => {}
        }
        if clobber_addresses {
            for local in address_taken {
                self.forget(LiveValue::Local(*local));
            }
        }
        // Calls and unboxing can also write a local result slot.
        for definition in instruction_defs(instruction) {
            self.forget(definition);
        }
        if let Some((definition, value)) = result {
            self.values.insert(definition, value);
        }
        if let Some((definition, target)) = address {
            self.addresses.insert(definition, target);
        }
    }
}
