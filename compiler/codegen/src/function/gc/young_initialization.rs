//! Skip initialization barriers only for an actual, uninterrupted TLAB result.

use super::allocation_initialization::InitializationAliases;
use super::*;

pub(in crate::function) struct YoungAllocation<'ctx> {
    aliases: InitializationAliases,
    condition: IntValue<'ctx>,
}

pub(in crate::function) fn single_predecessor_blocks(
    function: &scoop_lir::Function,
) -> HashSet<scoop_lir::BlockId> {
    let mut seen = HashSet::new();
    let mut shared = HashSet::from([function.entry]);
    let mut record = |target| {
        if !seen.insert(target) {
            shared.insert(target);
        }
    };
    for (_, block) in function.blocks.iter() {
        if let Some(Instruction::Invoke { site }) = block.instructions.last() {
            record(site.normal());
            record(site.unwind());
            continue;
        }
        match block.terminator {
            Terminator::Br(target) => record(target),
            Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => {
                record(then_block);
                record(else_block);
            }
            Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {
                continue;
            }
        }
    }
    seen.retain(|block| !shared.contains(block));
    seen
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn remember_young_allocation(&mut self, out: TempId, young: Option<IntValue<'ctx>>) {
        self.young_allocation = young
            .filter(|condition| condition.get_zero_extended_constant() != Some(0))
            .map(|condition| {
                self.prepare_initialization_aliases();
                YoungAllocation {
                    aliases: InitializationAliases::new(out),
                    condition,
                }
            });
    }

    pub(in crate::function) fn retain_young_allocation(&mut self, instruction: &Instruction) {
        let Some(allocation) = &mut self.young_allocation else {
            return;
        };
        let addressed = self
            .allocation_addressed_locals
            .as_ref()
            .expect("allocation aliases have collected addressed locals");
        if !allocation.aliases.advance(instruction, addressed) {
            self.young_allocation = None;
        }
    }

    pub(super) fn young_initialization(&self, object: Value) -> Option<IntValue<'ctx>> {
        self.young_allocation
            .as_ref()
            .filter(|allocation| allocation.aliases.contains(object))
            .map(|allocation| allocation.condition)
    }

    pub(in crate::function) fn allocated_value_barrier(
        &mut self,
        address: PointerValue<'ctx>,
        ty: &LirType,
        young: Option<IntValue<'ctx>>,
    ) -> Result<(), CodegenError> {
        let mut references = Vec::new();
        self.heap_reference_offsets(ty, 0, &mut references)?;
        if references.is_empty() {
            return Ok(());
        }
        references.sort_unstable();
        self.initialization_barrier(young, |emitter| {
            emitter.heap_reference_barrier(address, &references)
        })
    }

    pub(in crate::function) fn initialization_barrier(
        &mut self,
        young: Option<IntValue<'ctx>>,
        barrier: impl FnOnce(&mut Self) -> Result<(), CodegenError>,
    ) -> Result<(), CodegenError> {
        let Some(young) = young else {
            return barrier(self);
        };
        let slow = self
            .context
            .append_basic_block(self.llvm_function, "init.barrier");
        let done = self
            .context
            .append_basic_block(self.llvm_function, "init.continue");
        self.builder
            .build_conditional_branch(young, done, slow)
            .map_err(|error| CodegenError(format!("branch on young initialization: {error}")))?;
        self.builder.position_at_end(slow);
        self.field_barrier = None;
        barrier(self)?;
        self.builder
            .build_unconditional_branch(done)
            .map_err(|error| CodegenError(format!("leave initialization barrier: {error}")))?;
        self.builder.position_at_end(done);
        self.field_barrier = None;
        Ok(())
    }
}
