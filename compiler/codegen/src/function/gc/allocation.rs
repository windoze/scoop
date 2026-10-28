//! Managed TLAB allocation fast path and collecting slow path.

use super::*;

/// One Immix block minus its reserved first line (runtime spec 3.1).
const REGULAR_MANAGED_OBJECT_MAX_SIZE: u64 = 32768 - 128;

pub(in crate::function) struct AllocationResult<'ctx> {
    pub object: PointerValue<'ctx>,
    /// Only the actual TLAB branch proves nursery membership.
    pub young: Option<IntValue<'ctx>>,
}

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Bump through the thread's shared TLAB view. Allocations with known
    /// layouts finish inline in release; other bumps use the GC-leaf
    /// helper. Failed bumps retain the collecting slow path and its roots.
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
        let layout = self.known_allocation_layout(*descriptor);
        let descriptor = self.value(*descriptor)?.into_pointer_value();
        let requested_size = self.value(*requested_size)?.into_int_value();
        let allocation = self.managed_alloc_value(
            descriptor,
            requested_size,
            layout,
            safepoint,
            live,
            Some(out),
        )?;
        self.remember_young_allocation(out, allocation.young);
        self.temps.insert(out, allocation.object.into());
        Ok(())
    }

    pub(in crate::function) fn managed_alloc_value(
        &mut self,
        descriptor: PointerValue<'ctx>,
        requested_size: IntValue<'ctx>,
        layout: Option<AllocationLayout>,
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
        initialization_result: Option<TempId>,
    ) -> Result<AllocationResult<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let managed_ptr = managed_ptr_ty(context, self.managed_address_space);
        let i64_ty = context.i64_type();

        let (alignment, nursery_eligible) = self.allocation_parameters(descriptor, layout)?;
        let layout = layout.filter(|_| self.optimize_allocations);
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
        if self.reuse_thread_addresses
            && ([nursery_eligible, size_valid]
                .into_iter()
                .any(|condition| condition.get_zero_extended_constant() == Some(0))
                || aligned_size
                    .get_zero_extended_constant()
                    .is_some_and(|size| size > REGULAR_MANAGED_OBJECT_MAX_SIZE))
        {
            return Ok(AllocationResult {
                object: self.slow_allocation(descriptor, requested_size, safepoint, live)?,
                young: None,
            });
        }

        let allocation_context = self.allocation_context()?;
        let allocation_ty = super::allocation_metadata::allocation_context_type(context);
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
        let direct_bump = self.optimize_allocations
            && self.optimize_tlab_range
            && alignment.get_zero_extended_constant() == Some(8)
            && aligned_size
                .get_zero_extended_constant()
                .is_some_and(|size| (16..=REGULAR_MANAGED_OBJECT_MAX_SIZE).contains(&size));
        let object_int = if direct_bump {
            // Every live cursor is already header-aligned.
            cursor_int
        } else {
            builder
                .build_int_add(cursor_int, alignment_mask, "tlab_cursor_plus_align")
                .and_then(|cursor| builder.build_and(cursor, inverse_mask, "tlab_aligned_cursor"))
                .map_err(|error| CodegenError(format!("align TLAB object start: {error}")))?
        };
        let next_int = builder
            .build_int_add(object_int, aligned_size, "tlab_next_int")
            .map_err(|error| CodegenError(format!("advance TLAB object: {error}")))?;
        let fast = if direct_bump {
            // A retired view has both pointers null and therefore no capacity.
            let remaining = builder
                .build_int_sub(limit_int, cursor_int, "tlab_remaining")
                .map_err(|error| CodegenError(format!("remaining TLAB bytes: {error}")))?;
            let fits = builder
                .build_int_compare(
                    IntPredicate::ULE,
                    aligned_size,
                    remaining,
                    "alloc_within_tlab",
                )
                .map_err(|error| CodegenError(format!("check TLAB capacity: {error}")))?;
            let is_regular = builder
                .build_int_compare(
                    IntPredicate::ULE,
                    aligned_size,
                    i64_ty.const_int(REGULAR_MANAGED_OBJECT_MAX_SIZE, false),
                    "alloc_is_regular",
                )
                .map_err(|error| CodegenError(format!("check regular allocation: {error}")))?;
            builder
                .build_and(fits, is_regular, "alloc_has_regular_tlab")
                .and_then(|condition| {
                    builder.build_and(condition, nursery_eligible, "alloc_young_tlab")
                })
                .and_then(|condition| builder.build_and(condition, size_valid, "alloc_fast_path"))
        } else {
            let present = builder
                .build_int_compare(
                    IntPredicate::NE,
                    object_int,
                    i64_ty.const_zero(),
                    "tlab_present",
                )
                .map_err(|error| CodegenError(format!("check TLAB presence: {error}")))?;
            let is_regular = builder
                .build_int_compare(
                    IntPredicate::ULE,
                    aligned_size,
                    i64_ty.const_int(REGULAR_MANAGED_OBJECT_MAX_SIZE, false),
                    "alloc_is_regular",
                )
                .map_err(|error| CodegenError(format!("check regular allocation: {error}")))?;
            let within_limit = builder
                .build_int_compare(IntPredicate::ULE, next_int, limit_int, "alloc_within_tlab")
                .map_err(|error| CodegenError(format!("check TLAB limit: {error}")))?;
            builder
                .build_and(present, is_regular, "alloc_has_regular_tlab")
                .and_then(|condition| {
                    builder.build_and(condition, nursery_eligible, "alloc_young_tlab")
                })
                .and_then(|condition| {
                    builder.build_and(condition, within_limit, "alloc_within_limit")
                })
                .and_then(|condition| builder.build_and(condition, size_valid, "alloc_fast_path"))
        }
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
        let next = builder
            .build_int_to_ptr(next_int, ptr, "tlab_next")
            .map_err(|error| CodegenError(format!("materialize TLAB cursor: {error}")))?;
        builder
            .build_store(cursor_slot, next)
            .map_err(|error| CodegenError(format!("publish TLAB cursor: {error}")))?;
        if layout.is_some_and(|layout| layout.nursery_eligible) {
            let clear_size = if let (Some(out), Some(size)) = (
                initialization_result,
                aligned_size.get_zero_extended_constant(),
            ) {
                i64_ty.const_int(self.initialization_clear_size(out, size)?, false)
            } else {
                aligned_size
            };
            self.finish_inline_allocation(
                allocation_context,
                object,
                descriptor,
                aligned_size,
                clear_size,
            )?;
        } else {
            self.finish_allocation_call(object, descriptor, aligned_size)?;
        }
        let fast_end = builder
            .get_insert_block()
            .expect("allocation completion has an active block");
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave TLAB fast path: {error}")))?;

        builder.position_at_end(slow_block);
        let slow_object = self.slow_allocation(descriptor, requested_size, safepoint, live)?;
        builder
            .build_unconditional_branch(continue_block)
            .map_err(|error| CodegenError(format!("leave allocation slow path: {error}")))?;

        builder.position_at_end(continue_block);
        let phi = builder
            .build_phi(managed_ptr, "managed_object")
            .map_err(|error| CodegenError(format!("merge allocation result: {error}")))?;
        phi.add_incoming(&[(&object, fast_end), (&slow_object, slow_block)]);
        Ok(AllocationResult {
            object: phi.as_basic_value().into_pointer_value(),
            young: self.optimize_allocations.then_some(fast),
        })
    }

    fn slow_allocation(
        &mut self,
        descriptor: PointerValue<'ctx>,
        requested_size: IntValue<'ctx>,
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let context = self.context;
        let slow = self.runtime_fn(
            scoop_lir::RuntimeAbiSymbolV1::AllocateSlow.logical_symbol(),
            managed_ptr_ty(context, self.managed_address_space)
                .fn_type(&[ptr_ty(context).into(), context.i64_type().into()], false),
        );
        slow.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        let slow_call = self
            .builder
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
        Ok(slow_object)
    }

    fn finish_allocation_call(
        &self,
        object: PointerValue<'ctx>,
        descriptor: PointerValue<'ctx>,
        aligned_size: IntValue<'ctx>,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let finish = self.runtime_fn(
            scoop_lir::RuntimeAbiSymbolV1::FinishTlabAllocation.logical_symbol(),
            context.void_type().fn_type(
                &[
                    managed_ptr_ty(context, self.managed_address_space).into(),
                    ptr_ty(context).into(),
                    context.i64_type().into(),
                ],
                false,
            ),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        finish.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute("gc-leaf-function", ""),
        );
        self.builder
            .build_call(
                finish,
                &[object.into(), descriptor.into(), aligned_size.into()],
                "",
            )
            .map_err(|error| CodegenError(format!("finish TLAB allocation: {error}")))?;
        Ok(())
    }
}
