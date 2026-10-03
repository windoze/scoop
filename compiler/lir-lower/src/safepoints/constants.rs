use std::collections::HashMap;

use super::*;

/// Fold the boolean copies introduced by argument/default materialization
/// before LLVM promotes their slots and removes the untaken safepoints.
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
    for block in function.blocks.values_mut() {
        let lir::Terminator::CondBr {
            cond,
            then_block,
            else_block,
        } = block.terminator
        else {
            continue;
        };
        let mut constants = BlockConstants::default();
        for instruction in &block.instructions {
            constants.apply(instruction, &function.locals, &address_taken);
        }
        if let Some(value) = boolean_value(cond, &constants.values) {
            block.terminator = lir::Terminator::Br(if value { then_block } else { else_block });
        }
    }
}

fn known_result(
    instruction: &lir::Instruction,
    constants: &HashMap<LiveValue, bool>,
) -> Option<(LiveValue, bool)> {
    let (definition, value) = match *instruction {
        lir::Instruction::Store { local, value } => {
            (LiveValue::Local(local), boolean_value(value, constants)?)
        }
        lir::Instruction::UnaryOp {
            out,
            op: lir::UnOp::Not,
            operand,
        } => (LiveValue::Temp(out), !boolean_value(operand, constants)?),
        lir::Instruction::BinOp { out, op, lhs, rhs } => {
            let left = boolean_value(lhs, constants)?;
            let right = boolean_value(rhs, constants)?;
            let value = match op {
                lir::BinOp::Eq => left == right,
                lir::BinOp::Ne => left != right,
                lir::BinOp::MachineEq(_) => return None,
            };
            (LiveValue::Temp(out), value)
        }
        _ => return None,
    };
    Some((definition, value))
}

#[derive(Default)]
struct BlockConstants {
    values: HashMap<LiveValue, bool>,
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
                    result = boolean_value(*value, &self.values).map(|value| (destination, value));
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
                    result = boolean_value(source, &self.values).map(|value| (destination, value));
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

fn boolean_value(value: lir::Value, constants: &HashMap<LiveValue, bool>) -> Option<bool> {
    match value {
        lir::Value::BoolConst(value) => Some(value),
        lir::Value::Local(local) => constants.get(&LiveValue::Local(local)).copied(),
        lir::Value::Temp(temp) => constants.get(&LiveValue::Temp(temp)).copied(),
        _ => None,
    }
}
