//! Expose managed aggregate leaves after ordinary optimization, before RS4GC.
//!
//! InstCombine can undo an early split/rebuild and leave a stale aggregate
//! across a statepoint. Rebuild real consumers only after those passes finish.

use std::collections::HashMap;

use inkwell::llvm_sys::LLVMTypeKind;
use inkwell::llvm_sys::core::{
    LLVMGetGC, LLVMGetNormalDest, LLVMGetOperand, LLVMGetTypeKind, LLVMReplaceAllUsesWith,
    LLVMTypeOf,
};
use inkwell::values::{AggregateValueEnum, PhiValue};

use super::*;

mod components;
use components::{contains_managed, extract, insert, shape};

#[derive(Clone)]
struct Part<'ctx> {
    path: Vec<u32>,
    value: BasicValueEnum<'ctx>,
}

struct Splitter<'ctx> {
    builder: Builder<'ctx>,
    function: FunctionValue<'ctx>,
    space: ManagedAddressSpace,
    values: HashMap<BasicValueEnum<'ctx>, Vec<Part<'ctx>>>,
}

pub(super) fn expose(
    module: &LlvmModule<'_>,
    space: ManagedAddressSpace,
) -> Result<(), CodegenError> {
    for function in module.get_functions() {
        if function.count_basic_blocks() == 0
            || unsafe { LLVMGetGC(function.as_value_ref()) }.is_null()
            || function.get_gc().to_bytes() != GC_STRATEGY.as_bytes()
        {
            continue;
        }
        let mut splitter = Splitter {
            builder: module.get_context().create_builder(),
            function,
            space,
            values: HashMap::new(),
        };
        splitter.run()?;
    }
    Ok(())
}

impl<'ctx> Splitter<'ctx> {
    fn run(&mut self) -> Result<(), CodegenError> {
        // An invoke result is defined only on its normal edge. Give extracts
        // a dominating position even when the original destination is shared.
        let invokes = self
            .instructions()
            .into_iter()
            .filter(|instruction| {
                instruction.get_opcode() == InstructionOpcode::Invoke
                    && self.aggregate(instruction.as_value_ref()).is_some()
            })
            .collect::<Vec<_>>();
        for invoke in invokes {
            // SAFETY: this instruction is an invoke with a normal successor.
            let normal = unsafe { BasicBlock::new(LLVMGetNormalDest(invoke.as_value_ref())) }
                .expect("invoke normal block");
            super::edges::split(
                invoke.get_parent().expect("invoke parent"),
                normal,
                "gc.result.edge",
            )?;
        }
        let instructions = self.instructions();
        for instruction in &instructions {
            if let Some(value) = self.aggregate(instruction.as_value_ref()) {
                self.parts(value)?;
            }
        }
        let mut removed = Vec::new();
        for instruction in instructions {
            let opcode = instruction.get_opcode();
            let aggregate = self.aggregate(instruction.as_value_ref());
            if aggregate.is_some()
                && matches!(
                    opcode,
                    InstructionOpcode::Phi
                        | InstructionOpcode::Select
                        | InstructionOpcode::InsertValue
                        | InstructionOpcode::ExtractValue
                )
            {
                removed.push(instruction);
                continue;
            }
            if opcode == InstructionOpcode::ExtractValue {
                let source = operand(instruction, 0);
                if contains_managed(source.get_type(), self.space) {
                    let value = self.project(source, &instruction.get_indices())?;
                    // SAFETY: projection preserves the exact extract result type.
                    unsafe {
                        LLVMReplaceAllUsesWith(instruction.as_value_ref(), value.as_value_ref());
                    }
                    removed.push(instruction);
                    continue;
                }
            }
            for index in 0..instruction.get_num_operands() {
                // Do not convert token, block or metadata operands to basic values.
                let raw = unsafe { LLVMGetOperand(instruction.as_value_ref(), index) };
                let Some(value) = self.aggregate(raw) else {
                    continue;
                };
                let parts = self.parts(value)?;
                self.builder.position_before(&instruction);
                let mut rebuilt = value.get_type().const_zero();
                for part in parts {
                    rebuilt = insert(&self.builder, rebuilt, &part.path, part.value)?;
                }
                instruction.set_operand(index, rebuilt);
            }
        }
        // Pure aggregate cycles now have no real consumers. Break the old
        // graph before erasing it; scalar phis retain the actual incoming data.
        for instruction in &removed {
            // SAFETY: these are value-producing insert/extract/select/phi nodes.
            let value = unsafe { BasicValueEnum::new(instruction.as_value_ref()) };
            unsafe {
                LLVMReplaceAllUsesWith(
                    value.as_value_ref(),
                    value.get_type().const_zero().as_value_ref(),
                );
            }
        }
        for instruction in removed {
            instruction.erase_from_basic_block();
        }
        Ok(())
    }

    fn instructions(&self) -> Vec<InstructionValue<'ctx>> {
        self.function
            .get_basic_blocks()
            .into_iter()
            .flat_map(|block| block.get_instructions())
            .collect()
    }

    fn aggregate(
        &self,
        raw: inkwell::llvm_sys::prelude::LLVMValueRef,
    ) -> Option<BasicValueEnum<'ctx>> {
        if raw.is_null()
            || !matches!(
                unsafe { LLVMGetTypeKind(LLVMTypeOf(raw)) },
                LLVMTypeKind::LLVMStructTypeKind | LLVMTypeKind::LLVMArrayTypeKind
            )
        {
            return None;
        }
        // SAFETY: only concrete LLVM aggregate values reach this conversion.
        let value = unsafe { BasicValueEnum::new(raw) };
        contains_managed(value.get_type(), self.space).then_some(value)
    }

    fn parts(&mut self, value: BasicValueEnum<'ctx>) -> Result<Vec<Part<'ctx>>, CodegenError> {
        if let Some(parts) = self.values.get(&value) {
            return Ok(parts.clone());
        }
        let instruction = value.as_instruction_value();
        let parts = match instruction.map(|instruction| instruction.get_opcode()) {
            Some(InstructionOpcode::Phi) => {
                let instruction = instruction.unwrap();
                let old = PhiValue::try_from(instruction).expect("aggregate phi");
                self.builder.position_before(&instruction);
                let mut phis = Vec::new();
                let mut parts = Vec::new();
                for (path, ty) in shape(value.get_type(), self.space) {
                    let phi = self
                        .builder
                        .build_phi(ty, "gc.part")
                        .map_err(|error| CodegenError(format!("split aggregate phi: {error}")))?;
                    parts.push(Part {
                        path,
                        value: phi.as_basic_value(),
                    });
                    phis.push(phi);
                }
                self.values.insert(value, parts.clone());
                for (incoming, block) in old.get_incomings() {
                    let incoming = self.parts(incoming)?;
                    for (phi, part) in phis.iter().zip(incoming) {
                        phi.add_incoming(&[(&part.value, block)]);
                    }
                }
                return Ok(parts);
            }
            Some(InstructionOpcode::Select) => {
                let instruction = instruction.unwrap();
                let condition = operand(instruction, 0).into_int_value();
                let left = self.parts(operand(instruction, 1))?;
                let right = self.parts(operand(instruction, 2))?;
                self.builder.position_before(&instruction);
                left.into_iter()
                    .zip(right)
                    .map(|(left, right)| {
                        let value = self
                            .builder
                            .build_select(condition, left.value, right.value, "gc.part")
                            .map_err(|error| {
                                CodegenError(format!("split aggregate select: {error}"))
                            })?;
                        Ok(Part {
                            path: left.path,
                            value,
                        })
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?
            }
            Some(InstructionOpcode::ExtractValue) => {
                let instruction = instruction.unwrap();
                let path = instruction.get_indices();
                self.parts(operand(instruction, 0))?
                    .into_iter()
                    .filter(|part| part.path.starts_with(&path))
                    .map(|part| Part {
                        path: part.path[path.len()..].to_vec(),
                        value: part.value,
                    })
                    .collect()
            }
            Some(InstructionOpcode::InsertValue) => {
                let instruction = instruction.unwrap();
                let path = instruction.get_indices();
                let mut parts = self.parts(operand(instruction, 0))?;
                let inserted = operand(instruction, 1);
                for part in &mut parts {
                    if part.path.starts_with(&path) {
                        part.value = self.project(inserted, &part.path[path.len()..])?;
                    } else if path.starts_with(&part.path) {
                        self.builder.position_before(&instruction);
                        part.value = insert(
                            &self.builder,
                            part.value,
                            &path[part.path.len()..],
                            inserted,
                        )?;
                    }
                }
                parts
            }
            _ => {
                self.after_definition(value);
                shape(value.get_type(), self.space)
                    .into_iter()
                    .map(|(path, _)| {
                        let component = extract(&self.builder, value, &path)?;
                        Ok(Part {
                            path,
                            value: component,
                        })
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?
            }
        };
        self.values.insert(value, parts.clone());
        Ok(parts)
    }

    fn project(
        &mut self,
        value: BasicValueEnum<'ctx>,
        path: &[u32],
    ) -> Result<BasicValueEnum<'ctx>, CodegenError> {
        if path.is_empty() {
            if let Some(instruction) = value.as_instruction_value()
                && instruction.get_opcode() == InstructionOpcode::ExtractValue
                && contains_managed(operand(instruction, 0).get_type(), self.space)
            {
                return self.project(operand(instruction, 0), &instruction.get_indices());
            }
            return Ok(value);
        }
        let parts = self.parts(value)?;
        self.after_definition(value);
        for part in parts {
            if path.starts_with(&part.path) {
                return extract(&self.builder, part.value, &path[part.path.len()..]);
            }
        }
        Err(CodegenError(
            "aggregate projection does not select a component".into(),
        ))
    }

    fn after_definition(&self, value: BasicValueEnum<'ctx>) {
        let instruction = value.as_instruction_value();
        let block = match instruction {
            Some(instruction) if instruction.get_opcode() == InstructionOpcode::Invoke => {
                // SAFETY: all aggregate invokes have had their normal edges split.
                unsafe { BasicBlock::new(LLVMGetNormalDest(instruction.as_value_ref())) }.unwrap()
            }
            Some(instruction) if instruction.get_opcode() != InstructionOpcode::Phi => {
                self.builder.position_before(
                    &instruction
                        .get_next_instruction()
                        .expect("aggregate definition precedes its block terminator"),
                );
                return;
            }
            Some(instruction) => instruction.get_parent().unwrap(),
            None => self.function.get_first_basic_block().unwrap(),
        };
        let first = block
            .get_instructions()
            .find(|instruction| instruction.get_opcode() != InstructionOpcode::Phi)
            .expect("block terminator follows phis");
        self.builder.position_before(&first);
    }
}

fn operand(instruction: InstructionValue<'_>, index: u32) -> BasicValueEnum<'_> {
    instruction.get_operand(index).unwrap().value().unwrap()
}

fn aggregate(value: BasicValueEnum<'_>) -> AggregateValueEnum<'_> {
    match value {
        BasicValueEnum::StructValue(value) => value.into(),
        BasicValueEnum::ArrayValue(value) => value.into(),
        _ => unreachable!("extract/insert requires aggregate storage"),
    }
}
