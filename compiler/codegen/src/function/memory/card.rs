//! Inline sparse page-map lookup and marking, entirely between safepoints.
use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn card_mark(
        &self,
        addr: PointerValue<'ctx>,
    ) -> Result<(), CodegenError> {
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
        mark_typed_managed_pointer_boundary(
            context,
            address
                .as_instruction_value()
                .expect("heap ptrtoint is an instruction"),
            statepoint::TypedManagedPointerBoundary::CardAddress,
        )?;
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
        let offset = builder
            .build_int_sub(address, base, "card_offset")
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
        let slot = unsafe { builder.build_gep(context.i8_type(), cards, &[card], "card_ptr") }
            .map_err(card_error)?;
        builder
            .build_atomicrmw(
                AtomicRMWBinOp::Or,
                slot,
                context.i8_type().const_int(1, false),
                AtomicOrdering::Monotonic,
            )
            .map_err(card_error)?;
        Ok(())
    }
}

fn card_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("card mark: {error}"))
}
