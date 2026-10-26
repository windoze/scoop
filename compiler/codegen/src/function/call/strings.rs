use super::*;

mod compare;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn string_byte_length(
        &self,
        object: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        let length = self.byte_gep(object, 16, "string.length_ptr")?;
        self.builder
            .build_load(self.context.i64_type(), length, "string.byte_length")
            .map(BasicValueEnum::into_int_value)
            .map_err(string_error)
    }

    pub(super) fn string_byte_at(
        &self,
        object: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        length: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        // String lengths fit INT64_MAX, so this also rejects negative indices.
        let valid = self
            .builder
            .build_int_compare(IntPredicate::ULT, index, length, "string.index_valid")
            .map_err(string_error)?;
        let read = self
            .context
            .append_basic_block(self.llvm_function, "string.read");
        let invalid = self
            .context
            .append_basic_block(self.llvm_function, "string.invalid");
        self.builder
            .build_conditional_branch(valid, read, invalid)
            .map_err(string_error)?;

        self.builder.position_at_end(invalid);
        // Retain the runtime's exact fatal diagnostic on the invalid path.
        let callee = self.gc_leaf_fn(
            scoop_lir::RuntimeFunction::NoGc(scoop_lir::NoGcRuntimeFunction::StringByteAt).symbol(),
            self.context.i8_type().fn_type(
                &[object.get_type().into(), self.context.i64_type().into()],
                false,
            ),
        );
        let call = self
            .builder
            .build_call(callee, &[object.into(), index.into()], "")
            .map_err(string_error)?;
        self.apply_nounwind(call);
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
        );
        self.builder.build_unreachable().map_err(string_error)?;

        self.builder.position_at_end(read);
        let bytes = self.byte_gep(object, 24, "string.bytes")?;
        // SAFETY: the branch establishes an in-range byte of a valid String.
        let address = unsafe {
            self.builder
                .build_gep(self.context.i8_type(), bytes, &[index], "string.byte_ptr")
        }
        .map_err(string_error)?;
        self.builder
            .build_load(self.context.i8_type(), address, "string.byte")
            .map(BasicValueEnum::into_int_value)
            .map_err(string_error)
    }
}

fn string_error(error: inkwell::builder::BuilderError) -> CodegenError {
    CodegenError(format!("bounded String read: {error}"))
}
