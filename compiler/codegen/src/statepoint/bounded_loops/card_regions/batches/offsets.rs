//! Recognize constant byte increments without widening integer conversions.

use std::collections::HashMap;

use inkwell::targets::TargetData;
use inkwell::values::{BasicValueEnum, IntValue, PhiValue};

use super::{BasicValue, InstructionOpcode, InstructionValue, Region};

pub(super) struct Offsets<'a, 'ctx> {
    region: &'a Region<'ctx>,
    data: TargetData,
    steps: HashMap<InstructionValue<'ctx>, Option<i64>>,
}

impl<'a, 'ctx> Offsets<'a, 'ctx> {
    pub fn new(region: &'a Region<'ctx>, data: TargetData) -> Self {
        Self {
            region,
            data,
            steps: HashMap::new(),
        }
    }

    pub fn stride(&mut self, offset: IntValue<'ctx>) -> Option<u64> {
        if offset.get_type().get_bit_width() != 64 {
            return None;
        }
        u64::try_from(self.step(offset.as_basic_value_enum())?).ok()
    }

    fn step(&mut self, value: BasicValueEnum<'ctx>) -> Option<i64> {
        let Some(instruction) = value.as_instruction_value() else {
            return Some(0);
        };
        if !instruction
            .get_parent()
            .is_some_and(|block| self.region.blocks.contains(&block))
        {
            return Some(0);
        }
        if let Some(step) = self.steps.get(&instruction) {
            return *step;
        }
        self.steps.insert(instruction, None);
        let result = self.instruction(instruction);
        self.steps.insert(instruction, result);
        result
    }

    fn instruction(&mut self, instruction: InstructionValue<'ctx>) -> Option<i64> {
        let wide = instruction.get_type().is_int_type()
            && instruction.get_type().into_int_type().get_bit_width() == 64;
        match instruction.get_opcode() {
            InstructionOpcode::Phi if wide => self.induction(instruction),
            InstructionOpcode::Add | InstructionOpcode::Sub if wide => {
                let left = self.step(operand(instruction, 0)?)?;
                let right = self.step(operand(instruction, 1)?)?;
                if instruction.get_opcode() == InstructionOpcode::Add {
                    left.checked_add(right)
                } else {
                    left.checked_sub(right)
                }
            }
            InstructionOpcode::Mul if wide => {
                let left = operand(instruction, 0)?;
                let right = operand(instruction, 1)?;
                if let Some(scale) = constant(left) {
                    self.step(right)?.checked_mul(scale)
                } else if let Some(scale) = constant(right) {
                    self.step(left)?.checked_mul(scale)
                } else {
                    self.invariant(instruction)
                }
            }
            InstructionOpcode::Shl if wide => {
                let step = self.step(operand(instruction, 0)?)?;
                let shift = u32::try_from(constant(operand(instruction, 1)?)?).ok()?;
                step.checked_mul(1i64.checked_shl(shift).filter(|value| *value > 0)?)
            }
            InstructionOpcode::PtrToInt if wide => self.step(operand(instruction, 0)?),
            InstructionOpcode::Call if wide => self
                .step(crate::statepoint::address_bits::source(instruction)?.as_basic_value_enum()),
            InstructionOpcode::GetElementPtr => {
                let base = self.step(operand(instruction, 0)?)?;
                if instruction.get_num_operands() == 2 {
                    let index = self.step(operand(instruction, 1)?)?;
                    let element = instruction.get_gep_source_element_type().ok()?;
                    let size = i64::try_from(self.data.get_abi_size(&element)).ok()?;
                    base.checked_add(index.checked_mul(size)?)
                } else if instruction.get_operands().skip(1).all(|value| {
                    value
                        .and_then(|value| value.value())
                        .is_some_and(|value| self.step(value) == Some(0))
                }) {
                    Some(base)
                } else {
                    None
                }
            }
            InstructionOpcode::BitCast
            | InstructionOpcode::ZExt
            | InstructionOpcode::SExt
            | InstructionOpcode::Trunc
            | InstructionOpcode::And
            | InstructionOpcode::Or
            | InstructionOpcode::Xor
            | InstructionOpcode::LShr
            | InstructionOpcode::AShr
            | InstructionOpcode::ICmp
            | InstructionOpcode::Select => self.invariant(instruction),
            _ => None,
        }
    }

    fn invariant(&mut self, instruction: InstructionValue<'ctx>) -> Option<i64> {
        instruction
            .get_operands()
            .all(|operand| {
                operand
                    .and_then(|operand| operand.value())
                    .is_some_and(|value| self.step(value) == Some(0))
            })
            .then_some(0)
    }

    fn induction(&self, instruction: InstructionValue<'ctx>) -> Option<i64> {
        let phi = PhiValue::try_from(instruction).ok()?;
        if instruction.get_parent() != Some(self.region.header) || phi.count_incoming() != 2 {
            return None;
        }
        let mut initial = false;
        let mut step = None;
        for (value, predecessor) in phi.get_incomings() {
            if predecessor == self.region.predecessor {
                initial = !value
                    .as_instruction_value()
                    .and_then(|value| value.get_parent())
                    .is_some_and(|block| self.region.blocks.contains(&block));
            } else if predecessor == self.region.latch {
                let update = value.as_instruction_value()?;
                if !matches!(
                    update.get_opcode(),
                    InstructionOpcode::Add | InstructionOpcode::Sub
                ) {
                    return None;
                }
                let left = operand(update, 0)?;
                let right = operand(update, 1)?;
                step = match update.get_opcode() {
                    InstructionOpcode::Add if left == phi.as_basic_value() => constant(right),
                    InstructionOpcode::Add if right == phi.as_basic_value() => constant(left),
                    InstructionOpcode::Sub if left == phi.as_basic_value() => {
                        constant(right)?.checked_neg()
                    }
                    _ => None,
                };
            } else {
                return None;
            }
        }
        initial.then_some(step).flatten()
    }
}

fn operand(instruction: InstructionValue<'_>, index: u32) -> Option<BasicValueEnum<'_>> {
    instruction.get_operand(index)?.value()
}

fn constant(value: BasicValueEnum<'_>) -> Option<i64> {
    value
        .is_int_value()
        .then(|| value.into_int_value().get_sign_extended_constant())
        .flatten()
}
