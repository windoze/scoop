//! CFG paths bounded by dynamic SSA redefinitions, including PHI incoming edges.

use std::collections::{HashMap, HashSet};

use inkwell::llvm_sys::core::{
    LLVMCountIncoming, LLVMGetIncomingBlock, LLVMGetIncomingValue, LLVMGetNumSuccessors,
    LLVMGetSuccessor,
};
use inkwell::values::{AsValueRef, FunctionValue, InstructionOpcode, InstructionValue};

use crate::CodegenError;

/// Read-only positions and edges shared by all liveness queries in one function.
pub(super) struct FunctionFlow {
    positions: HashMap<usize, (usize, usize)>,
    successors: HashMap<usize, Vec<usize>>,
}

impl FunctionFlow {
    pub(super) fn new(function: FunctionValue<'_>) -> Result<Self, CodegenError> {
        let mut positions = HashMap::new();
        let mut successors = HashMap::new();
        for block in function.get_basic_blocks() {
            let key = block.as_mut_ptr() as usize;
            for (index, instruction) in block.get_instructions().enumerate() {
                positions.insert(instruction.as_value_ref() as usize, (key, index));
            }
            let terminator = block.get_terminator().ok_or_else(|| {
                CodegenError("LLVM basic block has no terminator after RS4GC".to_string())
            })?;
            let raw = terminator.as_value_ref();
            // SAFETY: these APIs accept a terminator from verified LLVM IR.
            let count = unsafe { LLVMGetNumSuccessors(raw) };
            successors.insert(
                key,
                (0..count)
                    .map(|index| unsafe { LLVMGetSuccessor(raw, index) } as usize)
                    .collect(),
            );
        }
        Ok(Self {
            positions,
            successors,
        })
    }

    pub(super) fn position(
        &self,
        instruction: InstructionValue<'_>,
    ) -> Result<(usize, usize), CodegenError> {
        self.positions
            .get(&(instruction.as_value_ref() as usize))
            .copied()
            .ok_or_else(|| {
                CodegenError(
                    "LLVM liveness query reached an instruction outside its function".to_string(),
                )
            })
    }

    fn successors(&self, block: usize) -> Result<&[usize], CodegenError> {
        self.successors
            .get(&block)
            .map(Vec::as_slice)
            .ok_or_else(|| {
                CodegenError("LLVM CFG successor belongs to another function".to_string())
            })
    }

    /// Re-entering the definition's block executes it again. That new dynamic
    /// value does not keep the pre-statepoint value live.
    pub(super) fn use_reachable_without_redefinition(
        &self,
        site: InstructionValue<'_>,
        user: InstructionValue<'_>,
        definition: InstructionValue<'_>,
    ) -> Result<bool, CodegenError> {
        if user.get_opcode() == InstructionOpcode::Phi {
            let raw = user.as_value_ref();
            let count = unsafe { LLVMCountIncoming(raw) };
            for index in 0..count {
                // SAFETY: `user` is a PHI in verified LLVM IR.
                if unsafe { LLVMGetIncomingValue(raw, index) } != definition.as_value_ref() {
                    continue;
                }
                let incoming_block = unsafe { LLVMGetIncomingBlock(raw, index) } as usize;
                if self.path_reaches_without_definition(site, incoming_block, None, definition)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        let (block, position) = self.position(user)?;
        self.path_reaches_without_definition(site, block, Some(position), definition)
    }

    /// Search to an instruction, or to a predecessor's outgoing PHI edge when
    /// `target_position` is absent, without re-executing `definition`.
    fn path_reaches_without_definition(
        &self,
        site: InstructionValue<'_>,
        target_block: usize,
        target_position: Option<usize>,
        definition: InstructionValue<'_>,
    ) -> Result<bool, CodegenError> {
        let (site_block, site_position) = self.position(site)?;
        let (definition_block, definition_position) = self.position(definition)?;
        // A block-local use after its definition cannot carry that value in
        // from another block: every incoming path re-executes the definition.
        if target_block == definition_block
            && target_position.is_none_or(|position| position > definition_position)
        {
            return Ok(site_block == definition_block
                && site_position >= definition_position
                && target_position.is_none_or(|position| position > site_position));
        }
        let mut pending = vec![(site_block, site_position + 1)];
        let mut visited = HashSet::new();
        while let Some((block, start)) = pending.pop() {
            if !visited.insert((block, start)) {
                continue;
            }
            let barrier = (block == definition_block && definition_position >= start)
                .then_some(definition_position);
            if block == target_block {
                match target_position {
                    Some(position)
                        if position >= start && barrier.is_none_or(|end| position < end) =>
                    {
                        return Ok(true);
                    }
                    None if barrier.is_none() => return Ok(true),
                    Some(_) | None => {}
                }
            }
            if barrier.is_none() {
                pending.extend(
                    self.successors(block)?
                        .iter()
                        .map(|&successor| (successor, 0)),
                );
            }
        }
        Ok(false)
    }

    pub(super) fn instruction_can_reach(
        &self,
        definition: InstructionValue<'_>,
        target: InstructionValue<'_>,
    ) -> Result<bool, CodegenError> {
        let (definition_block, definition_position) = self.position(definition)?;
        let (target_block, target_position) = self.position(target)?;
        if definition_block == target_block && definition_position < target_position {
            return Ok(true);
        }
        Ok(self
            .blocks_reachable_after(definition)?
            .contains(&target_block))
    }

    pub(super) fn blocks_reachable_after(
        &self,
        instruction: InstructionValue<'_>,
    ) -> Result<HashSet<usize>, CodegenError> {
        let (source, _) = self.position(instruction)?;
        let mut pending = self.successors(source)?.to_vec();
        let mut reachable = HashSet::new();
        while let Some(block) = pending.pop() {
            if reachable.insert(block) {
                pending.extend(self.successors(block)?);
            }
        }
        Ok(reachable)
    }
}

pub(super) fn phi_uses_value_after_statepoint(
    phi: InstructionValue<'_>,
    value: inkwell::llvm_sys::prelude::LLVMValueRef,
    site_block: usize,
    post_blocks: &HashSet<usize>,
) -> bool {
    let raw = phi.as_value_ref();
    // SAFETY: the caller only passes a PHI instruction from verified LLVM IR.
    let count = unsafe { LLVMCountIncoming(raw) };
    (0..count).any(|index| unsafe {
        LLVMGetIncomingValue(raw, index) == value
            && (LLVMGetIncomingBlock(raw, index) as usize == site_block
                || post_blocks.contains(&(LLVMGetIncomingBlock(raw, index) as usize)))
    })
}
