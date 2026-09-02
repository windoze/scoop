use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Get or declare a runtime function with the given signature.
    pub(super) fn runtime_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        self.llvm
            .get_function(symbol)
            .unwrap_or_else(|| self.llvm.add_function(symbol, ty, None))
    }

    /// Declare a helper which is proven not to enter Scoop GC or park the
    /// current managed segment. RS4GC must leave these calls untouched.
    pub(super) fn gc_leaf_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(symbol, ty);
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    pub(super) fn native_global_bridge(
        &self,
        symbol: &str,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(
            symbol,
            self.context
                .void_type()
                .fn_type(&[ptr_ty(self.context).into()], false),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        function.add_attribute(
            AttributeLoc::Function,
            self.context.create_string_attribute("gc-leaf-function", ""),
        );
        function
    }

    pub(super) fn emit_native_global_call(
        &self,
        callee: inkwell::values::FunctionValue<'ctx>,
        storage: PointerValue<'ctx>,
    ) -> Result<(), CodegenError> {
        self.builder
            .build_call(callee, &[storage.into()], "native_global_call")
            .map_err(|error| CodegenError(format!("native global call: {error}")))?;
        Ok(())
    }

    /// M13 managed allocation fast path. Small objects are bumped directly
    /// from the current thread's public two-pointer allocation context. A
    /// failed bump calls the collecting slow path; a successful bump calls a
    /// GC-leaf helper that clears the object, initializes its header, and
    /// atomically records the object start.
    pub(super) fn managed_alloc(
        &mut self,
        out: TempId,
        args: &[Value],
        safepoint: scoop_lir::SafepointId,
        live: MaterializedStatepointLive<'ctx>,
    ) -> Result<(), CodegenError> {
        let [descriptor, requested_size] = args else {
            return Err(CodegenError(format!(
                "scoop_rt_alloc @{}: expected descriptor and size",
                self.function.symbol
            )));
        };
        let descriptor = self.value(*descriptor)?.into_pointer_value();
        let requested_size = self.value(*requested_size)?.into_int_value();
        let object = self.managed_alloc_value(descriptor, requested_size, safepoint, live)?;
        self.temps.insert(out, object.into());
        Ok(())
    }

    pub(super) fn managed_alloc_value(
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

        let below_header = builder
            .build_int_compare(
                IntPredicate::ULT,
                requested_size,
                i64_ty.const_int(16, false),
                "alloc_below_header",
            )
            .map_err(|error| CodegenError(format!("allocation size check: {error}")))?;
        let at_least_header = builder
            .build_select(
                below_header,
                i64_ty.const_int(16, false),
                requested_size,
                "alloc_min_size",
            )
            .map_err(|error| CodegenError(format!("normalize allocation size: {error}")))?
            .into_int_value();
        let aligned_size = builder
            .build_and(
                builder
                    .build_int_add(
                        at_least_header,
                        i64_ty.const_int(7, false),
                        "alloc_size_plus_align",
                    )
                    .map_err(|error| CodegenError(format!("align allocation size: {error}")))?,
                i64_ty.const_int(!7_u64, false),
                "alloc_size",
            )
            .map_err(|error| CodegenError(format!("mask allocation size: {error}")))?;

        let allocation_global = self
            .llvm
            .get_global("scoop_rt_allocation_context")
            .unwrap_or_else(|| {
                let global = self
                    .llvm
                    .add_global(ptr, None, "scoop_rt_allocation_context");
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
            .and_then(|condition| builder.build_and(condition, within_limit, "alloc_fast_path"))
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
            "scoop_runtime_finish_tlab_alloc",
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
            "scoop_runtime_alloc_slow",
            managed_ptr.fn_type(&[ptr.into(), i64_ty.into()], false),
        );
        slow.add_attribute(
            AttributeLoc::Function,
            context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
        let slow_call = builder
            .build_call(
                slow,
                &[descriptor.into(), aligned_size.into()],
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

    pub(super) fn array_type(&self, id: ArrayTypeId) -> (&ArrayType, GlobalValue<'ctx>) {
        (&self.arrays[id], self.array_tds[arena_index(id)])
    }

    /// Byte-offset GEP from an opaque pointer (object field access).
    pub(super) fn byte_gep(
        &self,
        ptr: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: `ptr` addresses an object at least `offset` bytes
        // large (callers address fields of the runtime object model).
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                ptr,
                &[self.context.i32_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "gep {name} @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// M9 write-barrier instrumentation point (milestone9 DESIGN 3.1,
    /// runtime spec 3.6): after a heap store, mark the card covering the
    /// stored-to address with a monotonic atomic OR. Equal plain byte stores
    /// from multiple mutators would still be a data race.
    /// Emitted unconditionally (also for scalar stores); the v1
    /// collector ignores the table, and the generational remembered
    /// set consumes it once generations land.
    pub(super) fn card_mark(&self, addr: PointerValue<'ctx>) -> Result<(), CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let error = |e: inkwell::builder::BuilderError| {
            CodegenError(format!(
                "card mark @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        };
        // The card table is a runtime POINTER VARIABLE (`extern
        // unsigned char *scoop_gc_card_table`), pre-biased by the
        // runtime with the arena base so `base + (addr >> 9)` lands
        // inside the backing table for every heap address (see
        // runtime/include/scoop_rt.h): load the pointer, then GEP.
        let card_table_global = self.llvm.get_global(CARD_TABLE_SYMBOL).unwrap_or_else(|| {
            self.llvm
                .add_global(ptr_ty(context), None, CARD_TABLE_SYMBOL)
        });
        let card_table = builder
            .build_load(
                ptr_ty(context),
                card_table_global.as_pointer_value(),
                "card_table",
            )
            .map_err(error)?
            .into_pointer_value();
        let addr = builder
            .build_ptr_to_int(addr, context.i64_type(), "card_addr")
            .map_err(error)?;
        mark_typed_managed_pointer_boundary(
            context,
            addr.as_instruction_value()
                .expect("a non-constant ptrtoint is an instruction"),
            statepoint::TypedManagedPointerBoundary::CardAddress,
        )?;
        // Logical shift: the card index of the address.
        let card = builder
            .build_right_shift(
                addr,
                context.i64_type().const_int(CARD_SHIFT, false),
                false,
                "card_index",
            )
            .map_err(error)?;
        // SAFETY: the loaded card table base is pre-biased so that
        // `base + card` addresses the card of any heap address (one
        // card per 512 bytes of the GC window).
        let card_ptr =
            unsafe { builder.build_gep(context.i8_type(), card_table, &[card], "card_ptr") }
                .map_err(error)?;
        builder
            .build_atomicrmw(
                AtomicRMWBinOp::Or,
                card_ptr,
                context.i8_type().const_int(1, false),
                AtomicOrdering::Monotonic,
            )
            .map_err(error)?;
        Ok(())
    }

    /// Emit one explicit LIR poll. Its typed target, unique id and liveness
    /// plan were fixed by lir-lower; codegen does not inspect the CFG.
    pub(super) fn safepoint_poll(
        &mut self,
        site: &scoop_lir::ManagedPollSite,
    ) -> Result<(), CodegenError> {
        let target = self.function.call_targets.managed_targets.void[site.target].destination;
        if target
            != scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::Safepoint,
            )
        {
            return Err(CodegenError(format!(
                "managed poll @{} has a non-safepoint target",
                self.function.symbol
            )));
        }
        let live = self.materialize_statepoint_live(&site.live, site.safepoint)?;
        let safepoint = self.runtime_fn(
            scoop_lir::RuntimeFunction::Managed(scoop_lir::ManagedRuntimeFunction::Safepoint)
                .symbol(),
            self.context.void_type().fn_type(&[], false),
        );
        let call = self.builder.build_call(safepoint, &[], "").map_err(|e| {
            CodegenError(format!(
                "safepoint poll @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        self.apply_safepoint_id(call, site.safepoint);
        self.restore_statepoint_live(live, site.safepoint)?;
        Ok(())
    }

    /// Address of element `index` of an array object: the element
    /// area starts after the 16-byte header (M9) + size field, rounded
    /// up to the element type's ABI alignment.
    pub(super) fn element_ptr(
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
                symbol = self.function.symbol
            ))
        })
    }

    /// Emit the array bounds check: trap when `(u64)index >= (u64)size`
    /// (the unsigned comparison also rejects negative indexes, which
    /// wrap above every in-range size). On return the builder is
    /// positioned in the in-bounds continuation block; the rest of the
    /// current LIR block (including its terminator) is emitted there.
    pub(super) fn bounds_check(
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
                    symbol = self.function.symbol
                ))
            })?
            .into_int_value();
        let out_of_bounds = builder
            .build_int_compare(IntPredicate::UGE, index, size, "out_of_bounds")
            .map_err(|e| {
                CodegenError(format!(
                    "bounds check @{symbol}: {e}",
                    symbol = self.function.symbol
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
                    symbol = self.function.symbol
                ))
            })?;
        builder.position_at_end(ok_block);
        Ok(())
    }

    /// The shared bounds-check trap block of this function, created on
    /// first use: `scoop_rt_trap("array index out of bounds")` followed
    /// by `unreachable` (the TRAP_SYMBOL contract: noreturn).
    pub(super) fn bounds_trap_block(
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
            scoop_lir::TRAP_SYMBOL,
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
                    symbol = self.function.symbol
                ))
            })?;
        builder.build_unreachable().map_err(|e| {
            CodegenError(format!(
                "bounds trap @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })?;
        builder.position_at_end(current);
        self.bounds_trap_block = Some(block);
        Ok(block)
    }

    /// Address of the tag field of a tagged enum value in memory.
    pub(super) fn tag_ptr(
        &self,
        slot: PointerValue<'ctx>,
        ty: inkwell::types::StructType<'ctx>,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: constant indexes 0, 0 address the i64 tag of the
        // `{ i64, [M x i8] }` object `slot` points to.
        unsafe {
            self.builder.build_gep(
                ty,
                slot,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "tag_ptr",
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "tag gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }

    /// Address of one tagged-enum field at its enum-relative byte
    /// offset. LIR owns slot assignment and natural field layout.
    pub(super) fn enum_field_ptr(
        &self,
        slot: PointerValue<'ctx>,
        offset: u64,
        name: &str,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        // SAFETY: LIR guarantees every field offset lies within the
        // complete tagged-enum storage represented by `slot`.
        unsafe {
            self.builder.build_gep(
                self.context.i8_type(),
                slot,
                &[self.context.i64_type().const_int(offset, false)],
                name,
            )
        }
        .map_err(|e| {
            CodegenError(format!(
                "enum field gep @{symbol}: {e}",
                symbol = self.function.symbol
            ))
        })
    }
}
