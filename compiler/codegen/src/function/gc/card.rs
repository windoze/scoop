//! Inline mapping lookup and card marking, entirely between safepoints.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct CardRegion<'ctx> {
    mapping: PointerValue<'ctx>,
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
        let mapping = self.page_region_mapping(addr)?;
        self.card_region_from_mapping(mapping)
    }

    pub(super) fn array_card_region(
        &self,
        array: PointerValue<'ctx>,
    ) -> Result<CardRegion<'ctx>, CodegenError> {
        if !self.direct_array_regions {
            return self.card_region(array);
        }
        let address = self
            .builder
            .build_ptr_to_int(array, self.context.i64_type(), "array.owner_address")
            .map_err(card_error)?;
        let mapping = crate::gc_regions::array_mapping(self.llvm, self.builder, address)?;
        self.card_region_from_mapping(mapping)
    }

    fn page_region_mapping(
        &self,
        addr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let address = self
            .builder
            .build_ptr_to_int(addr, self.context.i64_type(), "card_addr")
            .map_err(card_error)?;
        crate::gc_regions::page_mapping(self.llvm, self.builder, address)
    }

    fn card_region_from_mapping(
        &self,
        node: PointerValue<'ctx>,
    ) -> Result<CardRegion<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let i64_ty = context.i64_type();
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
        Ok(CardRegion {
            mapping: node,
            base,
            cards,
        })
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
        let record = self.gc_leaf_fn(
            scoop_lir::RuntimeAbiSymbolV1::RecordDirtyCardGroup.logical_symbol(),
            context
                .void_type()
                .fn_type(&[ptr_ty(context).into(), i64_ty.into()], false),
        );
        record.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        builder
            .build_call(record, &[region.mapping.into(), card.into()], "")
            .map_err(card_error)?;
        Ok(())
    }
}

fn card_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("card mark: {error}"))
}
