use std::collections::HashMap;

use super::*;

/// Fold typed Boolean/integer copies before LLVM promotes their slots and
/// removes untaken branches that would otherwise retain safepoint records.
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
    constants: &HashMap<LiveValue, lir::Value>,
) -> Option<(LiveValue, lir::Value)> {
    let (definition, value) = match *instruction {
        lir::Instruction::Store { local, value } => {
            (LiveValue::Local(local), constant_value(value, constants)?)
        }
        lir::Instruction::UnaryOp {
            out,
            op: lir::UnOp::Not,
            operand,
        } => (
            LiveValue::Temp(out),
            lir::Value::BoolConst(!boolean_value(operand, constants)?),
        ),
        lir::Instruction::BinOp { out, op, lhs, rhs } => {
            let left = boolean_value(lhs, constants)?;
            let right = boolean_value(rhs, constants)?;
            let value = match op {
                lir::BinOp::Eq => left == right,
                lir::BinOp::Ne => left != right,
                lir::BinOp::MachineEq(_) => return None,
            };
            (LiveValue::Temp(out), lir::Value::BoolConst(value))
        }
        lir::Instruction::IntegerCompareTo {
            out,
            operand_kind,
            lhs,
            rhs,
        } => {
            let lir::Value::IntegerConst(left) = constant_value(lhs, constants)? else {
                return None;
            };
            let lir::Value::IntegerConst(right) = constant_value(rhs, constants)? else {
                return None;
            };
            let value: i64 = match integer_order(operand_kind, left, right)? {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            (
                LiveValue::Temp(out),
                lir::Value::IntegerConst(lir::LirIntegerConstant::Signed64(value as u64)),
            )
        }
        lir::Instruction::IntegerCompare {
            out,
            kind,
            comparison,
            lhs,
            rhs,
        } => {
            let lir::Value::IntegerConst(left) = constant_value(lhs, constants)? else {
                return None;
            };
            let lir::Value::IntegerConst(right) = constant_value(rhs, constants)? else {
                return None;
            };
            let order = integer_order(kind, left, right)?;
            let value = match comparison {
                lir::IntegerComparison::Less => order.is_lt(),
                lir::IntegerComparison::LessOrEqual => !order.is_gt(),
                lir::IntegerComparison::Greater => order.is_gt(),
                lir::IntegerComparison::GreaterOrEqual => !order.is_lt(),
                lir::IntegerComparison::Equal => order.is_eq(),
                lir::IntegerComparison::NotEqual => !order.is_eq(),
            };
            (LiveValue::Temp(out), lir::Value::BoolConst(value))
        }
        _ => return None,
    };
    Some((definition, value))
}

#[derive(Default)]
struct BlockConstants {
    values: HashMap<LiveValue, lir::Value>,
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
                    result = constant_value(*value, &self.values).map(|value| (destination, value));
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
                    result = constant_value(source, &self.values).map(|value| (destination, value));
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

fn constant_value(
    value: lir::Value,
    constants: &HashMap<LiveValue, lir::Value>,
) -> Option<lir::Value> {
    match value {
        lir::Value::BoolConst(_) | lir::Value::IntegerConst(_) => Some(value),
        lir::Value::Local(local) => constants.get(&LiveValue::Local(local)).copied(),
        lir::Value::Temp(temp) => constants.get(&LiveValue::Temp(temp)).copied(),
        _ => None,
    }
}

fn boolean_value(value: lir::Value, constants: &HashMap<LiveValue, lir::Value>) -> Option<bool> {
    match constant_value(value, constants)? {
        lir::Value::BoolConst(value) => Some(value),
        _ => None,
    }
}

fn integer_order(
    kind: lir::IntegerKind,
    left: lir::LirIntegerConstant,
    right: lir::LirIntegerConstant,
) -> Option<std::cmp::Ordering> {
    if left.kind() != kind || right.kind() != kind {
        return None;
    }
    let (left, right) = (left.raw_bits(), right.raw_bits());
    Some(match kind.signedness() {
        lir::IntegerSignedness::Unsigned => left.cmp(&right),
        lir::IntegerSignedness::Signed => {
            let shift = 64 - kind.width().bits();
            (((left << shift) as i64) >> shift).cmp(&(((right << shift) as i64) >> shift))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lir::LirIntegerConstant as C;
    use std::cmp::Ordering;

    #[test]
    fn comparisons_preserve_width_and_signedness_at_the_sign_bit() {
        for (left, right) in [
            (C::Signed8(0x80), C::Signed8(0x7f)),
            (C::Signed16(0x8000), C::Signed16(0x7fff)),
            (C::Signed32(0x8000_0000), C::Signed32(0x7fff_ffff)),
            (C::Signed64(1 << 63), C::Signed64((1 << 63) - 1)),
        ] {
            assert_eq!(
                integer_order(left.kind(), left, right),
                Some(Ordering::Less)
            );
        }
        for (left, right) in [
            (C::Unsigned8(0x80), C::Unsigned8(0x7f)),
            (C::Unsigned16(0x8000), C::Unsigned16(0x7fff)),
            (C::Unsigned32(0x8000_0000), C::Unsigned32(0x7fff_ffff)),
            (C::Unsigned64(1 << 63), C::Unsigned64((1 << 63) - 1)),
        ] {
            assert_eq!(
                integer_order(left.kind(), left, right),
                Some(Ordering::Greater)
            );
        }
    }

    #[test]
    fn local_integer_copies_resolve_the_array_length_guard() {
        let local = lir::LocalId::from_raw(la_arena::RawIdx::from(0));
        let out = lir::TempId::from_raw(la_arena::RawIdx::from(0));
        let mut constants = HashMap::new();
        let store = lir::Instruction::Store {
            local,
            value: lir::Value::IntegerConst(C::Signed64(40)),
        };
        let (key, value) = known_result(&store, &constants).unwrap();
        constants.insert(key, value);
        let comparison = lir::Instruction::IntegerCompare {
            out,
            kind: lir::IntegerKind::SIGNED_64,
            comparison: lir::IntegerComparison::Less,
            lhs: lir::Value::Local(local),
            rhs: lir::Value::IntegerConst(C::Signed64(0)),
        };
        assert_eq!(
            known_result(&comparison, &constants),
            Some((LiveValue::Temp(out), lir::Value::BoolConst(false)))
        );
        let compare_to = lir::Instruction::IntegerCompareTo {
            out,
            operand_kind: lir::IntegerKind::SIGNED_64,
            lhs: lir::Value::Local(local),
            rhs: lir::Value::IntegerConst(C::Signed64(41)),
        };
        assert_eq!(
            known_result(&compare_to, &constants),
            Some((
                LiveValue::Temp(out),
                lir::Value::IntegerConst(C::Signed64(u64::MAX))
            ))
        );
        constants.clear();
        assert_eq!(known_result(&comparison, &constants), None);
    }
}
