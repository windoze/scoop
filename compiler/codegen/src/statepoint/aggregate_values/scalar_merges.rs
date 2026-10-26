//! Canonicalize pointer merges exposed by late aggregate decomposition.
//!
//! EarlyCSE does not merge duplicate phis. Running InstCombine here could
//! reconstruct an old aggregate across GC, so only rewrite scalar merges.

use super::*;

#[derive(Eq, Hash, PartialEq)]
enum Merge<'ctx> {
    Phi(Vec<(BasicBlock<'ctx>, BasicValueEnum<'ctx>)>),
    Select([BasicValueEnum<'ctx>; 3]),
}

pub(super) fn simplify(function: FunctionValue<'_>) {
    let mut blocks = function
        .get_basic_blocks()
        .into_iter()
        .map(|block| {
            block
                .get_instructions()
                .filter(|instruction| {
                    matches!(
                        instruction.get_opcode(),
                        InstructionOpcode::Phi | InstructionOpcode::Select
                    ) && instruction.get_type().is_pointer_type()
                })
                .collect::<Vec<_>>()
        })
        .filter(|instructions| !instructions.is_empty())
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for instructions in &mut blocks {
            let mut available = HashMap::new();
            instructions.retain(|instruction| {
                // SAFETY: only scalar pointer instructions were collected.
                let value = unsafe { BasicValueEnum::new(instruction.as_value_ref()) };
                let merge = match instruction.get_opcode() {
                    InstructionOpcode::Phi => {
                        let mut incoming = PhiValue::try_from(*instruction)
                            .expect("pointer phi")
                            .get_incomings()
                            .map(|(value, block)| (block, value))
                            .collect::<Vec<_>>();
                        incoming.sort_unstable_by_key(|(block, _)| block.as_mut_ptr() as usize);
                        Merge::Phi(incoming)
                    }
                    InstructionOpcode::Select => {
                        Merge::Select([0, 1, 2].map(|index| operand(*instruction, index)))
                    }
                    _ => unreachable!("only pointer merges were collected"),
                };
                let key = (value.get_type().as_type_ref(), merge);
                let replacement = key
                    .1
                    .single_input(value)
                    .or_else(|| available.get(&key).copied());
                if let Some(replacement) = replacement {
                    // SAFETY: the replacement has the same type and incoming
                    // semantics; an earlier same-block merge dominates all uses.
                    unsafe {
                        LLVMReplaceAllUsesWith(value.as_value_ref(), replacement.as_value_ref());
                    }
                    instruction.erase_from_basic_block();
                    changed = true;
                    false
                } else {
                    available.insert(key, value);
                    true
                }
            });
        }
        // Replacing a merge can expose equivalence in earlier loop phis.
        // Every changing round removes an instruction; no CFG or aggregate
        // rewrite is performed, and logical root markers keep their identities.
        if !changed {
            break;
        }
    }
}

impl<'ctx> Merge<'ctx> {
    fn single_input(&self, value: BasicValueEnum<'ctx>) -> Option<BasicValueEnum<'ctx>> {
        match self {
            Self::Phi(incoming) => {
                let mut others = incoming
                    .iter()
                    .map(|(_, incoming)| *incoming)
                    .filter(|incoming| *incoming != value);
                let first = others.next()?;
                others.all(|incoming| incoming == first).then_some(first)
            }
            Self::Select([_, left, right]) => (left == right).then_some(*left),
        }
    }
}
