//! Reuse one ordinary object's region inside a straight-line write segment.

use super::*;
use inkwell::basic_block::BasicBlock;

pub(in crate::function) struct FieldBarrierCache<'ctx> {
    object: Value,
    block: BasicBlock<'ctx>,
    region: super::card::CardRegion<'ctx>,
    marked_offset: u64,
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn retain_field_barrier(&mut self, instruction: &Instruction) {
        // A small allowlist also discards the cache for any future instruction.
        // In particular, local/raw writes, all calls, atomics and polls end it.
        if !matches!(
            instruction,
            Instruction::HeapStore { .. }
                | Instruction::HeapLoad { .. }
                | Instruction::IntegerUnary { .. }
                | Instruction::IntegerBinary { .. }
                | Instruction::IntegerCompare { .. }
                | Instruction::IntegerCompareTo { .. }
                | Instruction::IntegerShift { .. }
                | Instruction::IntegerConvert { .. }
                | Instruction::MakeAggregate { .. }
                | Instruction::MakeZstValue { .. }
                | Instruction::ExtractValue { .. }
        ) {
            self.field_barrier = None;
        }
    }

    pub(in crate::function) fn heap_field_store(
        &mut self,
        object: Value,
        offset: u64,
        value: Value,
        ty: &LirType,
    ) -> Result<(), CodegenError> {
        let pointer = self.value(object)?.into_pointer_value();
        let address = self.byte_gep(pointer, offset, "field_ptr")?;
        self.builder
            .build_store(address, self.value(value)?)
            .map_err(|error| CodegenError(format!("heap field store: {error}")))?;
        let mut references = Vec::new();
        self.heap_reference_offsets(ty, 0, &mut references)?;
        let [reference] = references.as_slice() else {
            if references.is_empty() {
                return Ok(());
            }
            self.field_barrier = None;
            references.sort_unstable();
            return self.heap_reference_barrier(address, &references);
        };
        let marked_offset = offset
            .checked_add(*reference)
            .ok_or_else(|| CodegenError("field reference offset overflows".into()))?;
        let address = self.byte_gep(address, *reference, "barrier_start")?;
        let block = self.builder.get_insert_block().expect("field store block");
        let region = match &self.field_barrier {
            Some(cached) if cached.object == object && cached.block == block => {
                if cached.marked_offset == marked_offset {
                    return Ok(());
                }
                cached.region
            }
            _ => self.card_region(address)?,
        };
        self.mark_card_in_region(address, region)?;
        self.field_barrier = Some(FieldBarrierCache {
            object,
            block,
            region,
            marked_offset,
        });
        Ok(())
    }
}
