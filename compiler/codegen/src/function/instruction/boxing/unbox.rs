use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_unbox(
        &mut self,
        object: Value,
        result: &UnboxResult,
    ) -> Result<(), CodegenError> {
        let object = self.value(object)?.into_pointer_value();
        let descriptor = self
            .value(Value::TypeDescriptor(result.descriptor()))?
            .into_pointer_value();
        let actual = self
            .builder
            .build_load(ptr_ty(self.context), object, "unbox.actual_td")
            .map_err(box_error)?
            .into_pointer_value();
        let exact = self
            .builder
            .build_int_compare(IntPredicate::EQ, actual, descriptor, "unbox.exact")
            .map_err(box_error)?;
        let fast = self
            .context
            .append_basic_block(self.llvm_function, "unbox.fast");
        let slow = self
            .context
            .append_basic_block(self.llvm_function, "unbox.slow");
        let done = self
            .context
            .append_basic_block(self.llvm_function, "unbox.done");
        self.builder
            .build_conditional_branch(exact, fast, slow)
            .map_err(box_error)?;

        self.builder.position_at_end(fast);
        if let UnboxResult::NonZero(place) = result {
            let payload = place.descriptor();
            let address = self.byte_gep(object, payload.payload_offset(), "unbox.payload")?;
            let layout = payload.value().layout();
            let alignment = layout.alignment().get() as u32;
            // Copy storage, including observable padding, into the typed local.
            self.builder
                .build_memcpy(
                    self.local_pointer(place.local())?,
                    alignment,
                    address,
                    alignment,
                    self.context
                        .i64_type()
                        .const_int(layout.size().get(), false),
                )
                .map_err(box_error)?;
        }
        self.builder
            .build_unconditional_branch(done)
            .map_err(box_error)?;

        self.builder.position_at_end(slow);
        match result {
            UnboxResult::ZeroSized { .. } => {
                self.box_leaf(
                    NoGcRuntimeFunction::UnboxZst,
                    &[object.into(), descriptor.into()],
                )?;
            }
            UnboxResult::NonZero(place) => {
                let destination = self.local_pointer(place.local())?;
                self.box_leaf(
                    NoGcRuntimeFunction::UnboxValue,
                    &[object.into(), descriptor.into(), destination.into()],
                )?;
            }
        }
        self.builder
            .build_unconditional_branch(done)
            .map_err(box_error)?;
        self.builder.position_at_end(done);
        if let UnboxResult::ZeroSized { descriptor, out } = result {
            let ty = basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                descriptor.value().representation().storage_type(),
            )?;
            self.temps.insert(*out, ty.const_zero());
        }
        Ok(())
    }
}
