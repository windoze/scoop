//! Array metadata, element addressing, and bounds checks.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn array_type(
        &self,
        id: ArrayTypeId,
    ) -> (&ArrayType, GlobalValue<'ctx>) {
        (&self.arrays[id], self.array_tds[arena_index(id)])
    }

    /// Address of element `index` of an array object: the element
    /// area starts after the 16-byte header (M9) + size field, rounded
    /// up to the element type's ABI alignment.
    pub(in crate::function) fn element_ptr(
        &self,
        array: PointerValue<'ctx>,
        element_ty: BasicTypeEnum<'ctx>,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let data_offset = array_data_offset(self.target_data.get_abi_alignment(&element_ty) as u64);
        let base = self.byte_gep(array, data_offset, "elements")?;
        // SAFETY: `base` addresses the element area of an array whose
        // elements have layout `element_ty`; `index` was bounds-checked
        // against the array size (or is a valid constant index).
        unsafe { self.builder.build_gep(element_ty, base, &[index], name) }.map_err(|e| {
            CodegenError(format!(
                "element gep @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })
    }

    pub(in crate::function) fn array_size_check(
        &mut self,
        overflow: inkwell::values::IntValue<'ctx>,
        continuation_name: &str,
    ) -> Result<(), CodegenError> {
        let continuation = self
            .context
            .append_basic_block(self.llvm_function, continuation_name);
        let trap = self.array_size_trap_block()?;
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

    /// Emit the array bounds check: trap when `(u64)index >= (u64)size`
    /// (the unsigned comparison also rejects negative indexes, which
    /// wrap above every in-range size). On return the builder is
    /// positioned in the in-bounds continuation block; the rest of the
    /// current LIR block (including its terminator) is emitted there.
    pub(in crate::function) fn bounds_check(
        &mut self,
        array: PointerValue<'ctx>,
        index: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let builder = self.builder;
        let size_ptr = self.byte_gep(array, 16, "size_ptr")?;
        let size = builder
            .build_load(self.context.i64_type(), size_ptr, "size")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol()
                ))
            })?
            .into_int_value();
        let out_of_bounds = builder
            .build_int_compare(IntPredicate::UGE, index, size, "out_of_bounds")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol()
                ))
            })?;
        let ok_block = self
            .context
            .append_basic_block(self.llvm_function, "in_bounds");
        let trap_block = self.bounds_trap_block()?;
        builder
            .build_conditional_branch(out_of_bounds, trap_block, ok_block)
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol()
                ))
            })?;
        builder.position_at_end(ok_block);
        Ok(())
    }

    /// The shared bounds-check trap block of this function, created on
    /// first use: `scoop_rt_trap("array index out of bounds")` followed
    /// by `unreachable` (the typed runtime trap contract is `noreturn`).
    pub(in crate::function) fn bounds_trap_block(
        &mut self,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, CodegenError> {
        if let Some(block) = self.bounds_trap_block {
            return Ok(block);
        }
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;

        let message = self
            .bounds_message
            .ok_or_else(|| {
                CodegenError("bounds check in a module without array types".to_string())
            })?
            .as_pointer_value();

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
            .append_basic_block(self.llvm_function, "bounds_trap");
        builder.position_at_end(block);
        builder
            .build_call(trap, &[message.into()], "trap")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds trap @{symbol}: {e}",
                    symbol = self.function.symbol()
                ))
            })?;
        builder.build_unreachable().map_err(|e| {
            CodegenError(format!(
                "bounds trap @{symbol}: {e}",
                symbol = self.function.symbol()
            ))
        })?;
        builder.position_at_end(current);
        self.bounds_trap_block = Some(block);
        Ok(block)
    }

    pub(in crate::function) fn array_size_trap_block(
        &mut self,
    ) -> Result<inkwell::basic_block::BasicBlock<'ctx>, CodegenError> {
        if let Some(block) = self.array_size_trap_block {
            return Ok(block);
        }
        let builder = self.builder;
        let current = builder
            .get_insert_block()
            .ok_or_else(|| CodegenError("builder has no insertion block".to_string()))?;
        let message = self
            .array_size_message
            .ok_or_else(|| {
                CodegenError("array assembly in a module without its trap message".to_string())
            })?
            .as_pointer_value();
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
            .build_call(trap, &[message.into()], "trap")
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
        self.array_size_trap_block = Some(block);
        Ok(block)
    }
}
