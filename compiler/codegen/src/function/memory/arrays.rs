//! Array metadata, element addressing, and checked assembly sizes.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn array_type(
        &self,
        id: ArrayTypeId,
    ) -> (&ArrayType, GlobalValue<'ctx>) {
        (&self.arrays[id], self.array_tds[arena_index(id)])
    }

    /// Address of element `index` of an array object: the element
    /// area and nonzero stride come from the checked LIR instance layout.
    pub(in crate::function) fn element_ptr(
        &self,
        array: PointerValue<'ctx>,
        layout: &scoop_lir::ArrayLayoutV1,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let scoop_lir::ArrayElementStorageKindV1::Inline { stride, .. } = layout.storage().kind()
        else {
            return Err(CodegenError(
                "zero-sized array elements have no payload address".to_string(),
            ));
        };
        let data_offset = layout.instance().inline_offset();
        let offset = self
            .builder
            .build_int_mul(
                index,
                self.context.i64_type().const_int(stride.get(), false),
                "element_offset",
            )
            .map_err(|error| CodegenError(format!("array element offset: {error}")))?;
        let base = self.byte_gep(array, data_offset, "elements")?;
        // SAFETY: `base` addresses the element area of an array whose
        // elements have the checked LIR storage; `index` was bounds-checked
        // against the array size (or is a valid constant index).
        unsafe {
            self.builder
                .build_gep(self.context.i8_type(), base, &[offset], name)
        }
        .map_err(|e| {
            CodegenError(format!(
                "element gep @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })
    }

    pub(in crate::function) fn checked_array_allocation_size(
        &mut self,
        layout: &scoop_lir::ArrayLayoutV1,
        total: IntValue<'ctx>,
        overflow_message: scoop_lir::GlobalId,
    ) -> Result<IntValue<'ctx>, CodegenError> {
        let builder = self.builder;
        let i64_ty = self.context.i64_type();
        let exceeds_layout = builder
            .build_int_compare(
                IntPredicate::UGT,
                total,
                i64_ty.const_int(layout.maximum_count(), false),
                "assembly_bytes_overflow",
            )
            .map_err(|error| CodegenError(format!("array allocation size check: {error}")))?;
        self.array_size_check(exceeds_layout, overflow_message, "assembly.size.bytes.ok")?;
        let total_bytes = match layout.storage().kind() {
            scoop_lir::ArrayElementStorageKindV1::ZeroSized { .. } => {
                i64_ty.const_int(layout.instance().minimum_size(), false)
            }
            scoop_lir::ArrayElementStorageKindV1::Inline { stride, .. } => {
                let bytes = builder
                    .build_int_mul(
                        total,
                        i64_ty.const_int(stride.get(), false),
                        "assembly_element_bytes",
                    )
                    .and_then(|bytes| {
                        builder.build_int_add(
                            bytes,
                            i64_ty.const_int(layout.instance().inline_offset(), false),
                            "assembly_total_bytes",
                        )
                    })
                    .map_err(|error| CodegenError(format!("array allocation bytes: {error}")))?;
                let mask = layout.instance().instance_alignment() - 1;
                builder
                    .build_int_add(bytes, i64_ty.const_int(mask, false), "assembly_size_round")
                    .and_then(|bytes| {
                        builder.build_and(
                            bytes,
                            i64_ty.const_int(!mask, false),
                            "assembly_aligned_size",
                        )
                    })
                    .map_err(|error| CodegenError(format!("array allocation alignment: {error}")))?
            }
        };
        Ok(total_bytes)
    }

    pub(in crate::function) fn array_size_check(
        &mut self,
        overflow: inkwell::values::IntValue<'ctx>,
        message: scoop_lir::GlobalId,
        continuation_name: &str,
    ) -> Result<(), CodegenError> {
        let continuation = self
            .context
            .append_basic_block(self.llvm_function, continuation_name);
        let trap = self.array_size_trap_block(message)?;
        self.builder
            .build_conditional_branch(overflow, trap, continuation)
            .map_err(|error| {
                CodegenError(format!(
                    "array size check @{symbol}: {error}",
                    symbol = self.function.symbol()
                ))
            })?;
        self.builder.position_at_end(continuation);
        Ok(())
    }

    pub(in crate::function) fn array_size_trap_block(
        &mut self,
        message: scoop_lir::GlobalId,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, CodegenError> {
        if let Some(&block) = self.array_size_trap_blocks.get(&message) {
            return Ok(block);
        }
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;
        let message_pointer = self.value(Value::Global(message))?.into_pointer_value();
        let trap = self.gc_leaf_fn(
            scoop_lir::RuntimeAbiSymbolV1::LirCall(scoop_lir::RuntimeFunction::NoGc(
                scoop_lir::NoGcRuntimeFunction::Trap,
            ))
            .logical_symbol(),
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        let block = self
            .context
            .append_basic_block(self.llvm_function, "array_size_trap");
        builder.position_at_end(block);
        builder
            .build_call(trap, &[message_pointer.into()], "trap")
            .map_err(|error| {
                CodegenError(format!(
                    "array size trap @{symbol}: {error}",
                    symbol = self.function.symbol()
                ))
            })?;
        builder.build_unreachable().map_err(|error| {
            CodegenError(format!(
                "array size trap @{symbol}: {error}",
                symbol = self.function.symbol()
            ))
        })?;
        builder.position_at_end(current);
        self.array_size_trap_blocks.insert(message, block);
        Ok(block)
    }
}
