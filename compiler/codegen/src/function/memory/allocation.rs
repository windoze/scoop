//! Managed TLAB allocation fast path and collecting slow path.

use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// M13 managed allocation fast path. Small objects are bumped directly
    /// from the current thread's public two-pointer allocation context. A
    /// failed bump calls the collecting slow path; a successful bump calls a
    /// GC-leaf helper that clears the object, initializes its header, and
    /// atomically records the object start.
    pub(in crate::function) fn managed_alloc(
        &mut self,
        out: TempId,
        args: &[Value],
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    ) -> Result<(), CodegenError> {
        let [descriptor, requested_size] = args else {
            return Err(CodegenError(format!(
                "scoop_rt_alloc @{}: expected descriptor and size",
                self.function.symbol()
            )));
        };
        let descriptor = self.value(*descriptor)?.into_pointer_value();
        let requested_size = self.value(*requested_size)?.into_int_value();
        let object = self.managed_alloc_value(descriptor, requested_size, safepoint, live)?;
        self.temps.insert(out, object.into());
        Ok(())
    }

    pub(in crate::function) fn managed_alloc_value(
        &mut self,
        descriptor: PointerValue<'ctx>,
        requested_size: IntValue<'ctx>,
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let managed_ptr = managed_ptr_ty(context, self.managed_address_space);
        let i64_ty = context.i64_type();

        let metadata = crate::runtime_metadata_v1::RuntimeMetadataV1Types::new(context);
        let shape = builder
            .build_struct_gep(
                metadata.type_descriptor(),
                descriptor,
                1,
                "allocation_shape",
            )
            .map_err(|error| CodegenError(format!("allocation descriptor shape: {error}")))?;
        let alignment_slot = builder
            .build_struct_gep(
                metadata.type_instance_shape(),
                shape,
                3,
                "allocation_alignment_ptr",
            )
            .map_err(|error| CodegenError(format!("allocation descriptor alignment: {error}")))?;
        let alignment = builder
            .build_load(i64_ty, alignment_slot, "allocation_alignment")
            .map_err(|error| CodegenError(format!("load allocation alignment: {error}")))?
            .into_int_value();
        let alignment_mask = builder
            .build_int_sub(
                alignment,
                i64_ty.const_int(1, false),
                "allocation_alignment_mask",
            )
            .map_err(|error| CodegenError(format!("allocation alignment mask: {error}")))?;
        let inverse_mask = builder
            .build_not(alignment_mask, "allocation_alignment_inverse")
            .map_err(|error| CodegenError(format!("allocation alignment mask: {error}")))?;
        let maximum_size = builder
            .build_int_sub(
                i64_ty.const_int(i64::MAX as u64, false),
                alignment_mask,
                "allocation_maximum_size",
            )
            .map_err(|error| CodegenError(format!("allocation maximum size: {error}")))?;
        let size_valid = builder
            .build_int_compare(
                IntPredicate::ULE,
                requested_size,
                maximum_size,
                "allocation_size_valid",
            )
            .map_err(|error| CodegenError(format!("allocation size overflow: {error}")))?;
        let aligned_size = builder
            .build_int_add(requested_size, alignment_mask, "alloc_size_plus_align")
            .and_then(|size| builder.build_and(size, inverse_mask, "alloc_size"))
            .map_err(|error| CodegenError(format!("allocation size alignment: {error}")))?;

        let allocation_context_symbol =
            scoop_lir::RuntimeAbiSymbolV1::AllocationContext.logical_symbol();
        let allocation_global = self
            .llvm
            .get_global(allocation_context_symbol)
            .unwrap_or_else(|| {
                let global = self.llvm.add_global(ptr, None, allocation_context_symbol);
                global.set_thread_local(true);
                global
            });
        let allocation_context = builder
            .build_load(
                ptr,
                allocation_global.as_pointer_value(),
                "allocation_context",
            )
            .map_err(|error| CodegenError(format!("load allocation context: {error}")))?
            .into_pointer_value();
        let allocation_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
        let cursor_slot = builder
            .build_struct_gep(allocation_ty, allocation_context, 0, "tlab_cursor_slot")
            .map_err(|error| CodegenError(format!("address TLAB cursor: {error}")))?;
        let limit_slot = builder
            .build_struct_gep(allocation_ty, allocation_context, 1, "tlab_limit_slot")
            .map_err(|error| CodegenError(format!("address TLAB limit: {error}")))?;
        let cursor = builder
            .build_load(ptr, cursor_slot, "tlab_cursor")
            .map_err(|error| CodegenError(format!("load TLAB cursor: {error}")))?
            .into_pointer_value();
        let limit = builder
            .build_load(ptr, limit_slot, "tlab_limit")
            .map_err(|error| CodegenError(format!("load TLAB limit: {error}")))?
            .into_pointer_value();
        let cursor_int = builder
            .build_ptr_to_int(cursor, i64_ty, "tlab_cursor_int")
            .map_err(|error| CodegenError(format!("convert TLAB cursor: {error}")))?;
        let limit_int = builder
            .build_ptr_to_int(limit, i64_ty, "tlab_limit_int")
            .map_err(|error| CodegenError(format!("convert TLAB limit: {error}")))?;
        let cursor_int = builder
            .build_int_add(cursor_int, alignment_mask, "tlab_cursor_plus_align")
            .and_then(|cursor| builder.build_and(cursor, inverse_mask, "tlab_aligned_cursor"))
            .map_err(|error| CodegenError(format!("align TLAB object start: {error}")))?;
        let cursor_end = builder
            .build_int_add(cursor_int, aligned_size, "tlab_cursor_end")
            .map_err(|error| CodegenError(format!("advance TLAB cursor: {error}")))?;
        let line_base = builder
            .build_and(
                cursor_int,
                i64_ty.const_int(!127_u64, false),
                "tlab_line_base",
            )
            .map_err(|error| CodegenError(format!("align TLAB line: {error}")))?;
        let line_end = builder
            .build_int_add(line_base, i64_ty.const_int(128, false), "tlab_line_end")
            .map_err(|error| CodegenError(format!("compute TLAB line end: {error}")))?;
        let fits_current_line = builder
            .build_int_compare(IntPredicate::ULE, cursor_end, line_end, "alloc_fits_line")
            .map_err(|error| CodegenError(format!("check TLAB line: {error}")))?;
        let object_int = builder
            .build_select(fits_current_line, cursor_int, line_end, "tlab_object_int")
            .map_err(|error| CodegenError(format!("select TLAB object: {error}")))?
            .into_int_value();
        let next_int = builder
            .build_int_add(object_int, aligned_size, "tlab_next_int")
            .map_err(|error| CodegenError(format!("advance selected TLAB object: {error}")))?;
        let has_tlab = builder
            .build_int_compare(
                IntPredicate::NE,
                cursor_int,
                i64_ty.const_zero(),
                "tlab_present",
            )
            .map_err(|error| CodegenError(format!("check TLAB presence: {error}")))?;
        let is_small = builder
            .build_int_compare(
                IntPredicate::ULE,
                aligned_size,
                i64_ty.const_int(64, false),
                "alloc_is_small",
            )
            .map_err(|error| CodegenError(format!("check small allocation: {error}")))?;
        let within_limit = builder
            .build_int_compare(IntPredicate::ULE, next_int, limit_int, "alloc_within_tlab")
            .map_err(|error| CodegenError(format!("check TLAB limit: {error}")))?;
        let fast = builder
            .build_and(has_tlab, is_small, "alloc_has_small_tlab")
            .and_then(|condition| builder.build_and(condition, within_limit, "alloc_within_limit"))
            .and_then(|condition| builder.build_and(condition, size_valid, "alloc_fast_path"))
            .map_err(|error| CodegenError(format!("combine TLAB checks: {error}")))?;

        let index = self.allocation_index;
        self.allocation_index += 1;
        let fast_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.fast.{index}"));
        let slow_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.slow.{index}"));
        let continue_block =
            context.append_basic_block(self.llvm_function, &format!("alloc.continue.{index}"));
        builder
            .build_conditional_branch(fast, fast_block, slow_block)
            .map_err(|error| CodegenError(format!("branch on TLAB fast path: {error}")))?;

        builder.position_at_end(fast_block);
        let object = builder
            .build_int_to_ptr(object_int, managed_ptr, "tlab_object")
            .map_err(|error| CodegenError(format!("materialize TLAB object: {error}")))?;
        mark_typed_managed_pointer_boundary(
            context,
            object
                .as_instruction_value()
                .expect("a non-constant inttoptr is an instruction"),
            statepoint::TypedManagedPointerBoundary::AllocationResult,
        )?;
        let next = builder
            .build_int_to_ptr(next_int, ptr, "tlab_next")
            .map_err(|error| CodegenError(format!("materialize TLAB cursor: {error}")))?;
        builder
            .build_store(cursor_slot, next)
            .map_err(|error| CodegenError(format!("publish TLAB cursor: {error}")))?;
        let finish = self.runtime_fn(
            scoop_lir::RuntimeAbiSymbolV1::FinishTlabAllocation.logical_symbol(),
            context
                .void_type()
                .fn_type(&[managed_ptr.into(), ptr.into(), i64_ty.into()], false),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute("gc-leaf-function", ""),
        );
        builder
            .build_call(
                finish,
                &[object.into(), descriptor.into(), aligned_size.into()],
                "",
            )
            .map_err(|error| CodegenError(format!("finish TLAB allocation: {error}")))?;
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave TLAB fast path: {error}")))?;

        builder.position_at_end(slow_block);
        let slow = self.runtime_fn(
            scoop_lir::RuntimeAbiSymbolV1::AllocateSlow.logical_symbol(),
            managed_ptr.fn_type(&[ptr.into(), i64_ty.into()], false),
        );
        slow.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        let slow_call = builder
            .build_call(
                slow,
                &[descriptor.into(), requested_size.into()],
                "slow_object",
            )
            .map_err(|error| CodegenError(format!("slow allocation: {error}")))?;
        self.apply_safepoint_id(slow_call, safepoint);
        let slow_object = slow_call
            .try_as_basic_value()
            .basic()
            .expect("allocation slow path returns an object")
            .into_pointer_value();
        self.restore_statepoint_live(live, safepoint)?;
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave allocation slow path: {error}")))?;

        builder.position_at_end(continue_block);
        let phi = builder
            .build_phi(managed_ptr, "managed_object")
            .map_err(|error| CodegenError(format!("merge allocation result: {error}")))?;
        phi.add_incoming(&[(&object, fast_block), (&slow_object, slow_block)]);
        Ok(phi.as_basic_value().into_pointer_value())
    }
}
