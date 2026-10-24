//! Inline sparse page-map lookup and marking, entirely between safepoints.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct CardRegion<'ctx> {
    base: IntValue<'ctx>,
    cards: PointerValue<'ctx>,
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn card_mark(&self, addr: PointerValue<'ctx>) -> Result<(), CodegenError> {
        let region = self.card_region(addr)?;
        self.mark_card_in_region(addr, region)
    }

    pub(super) fn card_region(
        &self,
        addr: PointerValue<'ctx>,
    ) -> Result<CardRegion<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let i64_ty = context.i64_type();
        let symbol = scoop_lir::RuntimeAbiSymbolV1::PageMap.logical_symbol();
        let root = self
            .llvm
            .get_global(symbol)
            .unwrap_or_else(|| self.llvm.add_global(ptr.array_type(4096), None, symbol));
        let address = builder
            .build_ptr_to_int(addr, i64_ty, "card_addr")
            .map_err(card_error)?;
        let mut node = root.as_pointer_value();
        for shift in [52, 40, 28, 16] {
            let shifted = builder
                .build_right_shift(address, i64_ty.const_int(shift, false), false, "page_shift")
                .map_err(card_error)?;
            let index = builder
                .build_and(shifted, i64_ty.const_int(4095, false), "page_index")
                .map_err(card_error)?;
            // SAFETY: every level contains 4096 pointer slots. A valid heap
            // destination has a fully published path through all four levels.
            let slot = unsafe { builder.build_gep(ptr, node, &[index], "page_slot") }
                .map_err(card_error)?;
            let loaded = builder
                .build_load(ptr, slot, "page_entry")
                .map_err(card_error)?;
            loaded
                .as_instruction_value()
                .expect("page-map load is an instruction")
                .set_atomic_ordering(AtomicOrdering::Acquire)
                .map_err(|error| CodegenError(format!("page-map acquire load: {error}")))?;
            node = loaded.into_pointer_value();
        }
        let prefix = context.struct_type(&[i64_ty.into(), i64_ty.into(), ptr.into()], false);
        let base_slot = builder
            .build_struct_gep(prefix, node, 0, "region_base_slot")
            .map_err(card_error)?;
        let cards_slot = builder
            .build_struct_gep(prefix, node, 2, "region_cards_slot")
            .map_err(card_error)?;
        let base = builder
            .build_load(i64_ty, base_slot, "region_base")
            .map_err(card_error)?
            .into_int_value();
        let cards = builder
            .build_load(ptr, cards_slot, "region_cards")
            .map_err(card_error)?
            .into_pointer_value();
        Ok(CardRegion { base, cards })
    }

    pub(super) fn mark_card_in_region(
        &self,
        addr: PointerValue<'ctx>,
        region: CardRegion<'ctx>,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let i64_ty = context.i64_type();
        let address = builder
            .build_ptr_to_int(addr, i64_ty, "card_addr")
            .map_err(card_error)?;
        let offset = builder
            .build_int_sub(address, region.base, "card_offset")
            .map_err(card_error)?;
        let card = builder
            .build_right_shift(
                offset,
                i64_ty.const_int(CARD_SHIFT, false),
                false,
                "card_index",
            )
            .map_err(card_error)?;
        // SAFETY: the destination belongs to this mapping and its cards cover
        // the entire immutable [base, base + size) range.
        let slot =
            unsafe { builder.build_gep(context.i8_type(), region.cards, &[card], "card_ptr") }
                .map_err(card_error)?;
        // Cards have only clean/dirty states. Clearing happens after mutators
        // stop, so concurrent stores of 1 need no read-modify-write operation.
        builder
            .build_store(slot, context.i8_type().const_int(1, false))
            .map_err(card_error)?
            .set_atomic_ordering(AtomicOrdering::Monotonic)
            .map_err(|error| CodegenError(format!("card mark atomic store: {error}")))?;
        Ok(())
    }
}

fn card_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("card mark: {error}"))
}
