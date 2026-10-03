use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(in crate::function) fn native_boundary_fn(
        &self,
        symbol: &str,
        ty: inkwell::types::FunctionType<'ctx>,
    ) -> inkwell::values::FunctionValue<'ctx> {
        let function = self.runtime_fn(symbol, ty);
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

    pub(in crate::function) fn publish_native_roots(
        &mut self,
        roots: &[scoop_lir::CallerRoot],
        result_root: Option<(PointerValue<'ctx>, &RefScan)>,
        kind: NativeTransitionKind,
        safepoint: scoop_lir::SafepointId,
    ) -> Result<NativeTransition<'ctx>, CodegenError> {
        let context = self.context;
        let builder = self.builder;
        let ptr = ptr_ty(context);
        let i64_ty = context.i64_type();
        let mut entries = Vec::with_capacity(roots.len() + usize::from(result_root.is_some()));
        for root in roots {
            let storage = self.root_source_storage(root.source)?;
            let descriptor = self
                .runtime_scans
                .emit(root.scan.as_ref_scan())?
                .ok_or_else(|| CodegenError("caller root has an empty runtime scan".to_string()))?;
            entries.push((storage.pointer, descriptor));
        }
        if let Some((storage, scan)) = result_root {
            let descriptor = self.runtime_scans.emit(scan)?.ok_or_else(|| {
                CodegenError("native result root has an empty runtime scan".to_string())
            })?;
            entries.push((storage, descriptor));
        }

        let entry_ty = context.struct_type(&[ptr.into(), ptr.into()], false);
        let entries_pointer = if entries.is_empty() {
            ptr.const_null()
        } else {
            let entry_count = u32::try_from(entries.len()).map_err(|_| {
                CodegenError("caller-root entry count exceeds u32::MAX".to_string())
            })?;
            let array_ty = entry_ty.array_type(entry_count);
            let array = self.entry_alloca(array_ty.into(), "caller_root_entries")?;
            for (index, (base, scan)) in entries.into_iter().enumerate() {
                let index = u64::try_from(index).map_err(|_| {
                    CodegenError("caller-root entry index exceeds u64::MAX".to_string())
                })?;
                // SAFETY: `index` is within the statically-sized entries array.
                let entry = unsafe {
                    builder.build_gep(
                        array_ty,
                        array,
                        &[
                            context.i32_type().const_zero(),
                            context.i32_type().const_int(index, false),
                        ],
                        "caller_root_entry",
                    )
                }
                .map_err(|error| CodegenError(format!("caller-root entry GEP: {error}")))?;
                let base_field = builder
                    .build_struct_gep(entry_ty, entry, 0, "caller_root_base")
                    .map_err(|error| CodegenError(format!("caller-root base GEP: {error}")))?;
                let scan_field = builder
                    .build_struct_gep(entry_ty, entry, 1, "caller_root_scan")
                    .map_err(|error| CodegenError(format!("caller-root scan GEP: {error}")))?;
                builder
                    .build_store(base_field, base)
                    .map_err(|error| CodegenError(format!("publish caller-root base: {error}")))?;
                builder
                    .build_store(scan_field, scan)
                    .map_err(|error| CodegenError(format!("publish caller-root scan: {error}")))?;
            }
            array
        };

        let frame_ty = context.struct_type(&[ptr.into(), ptr.into(), i64_ty.into()], false);
        let frame = self.entry_alloca(frame_ty.into(), "caller_root_frame")?;
        builder
            .build_store(frame, frame_ty.const_zero())
            .map_err(|error| CodegenError(format!("zero caller-root frame: {error}")))?;
        let push = self.native_boundary_fn(
            scoop_lir::RuntimeAbiSymbolV1::PushCallerRoots.logical_symbol(),
            context
                .void_type()
                .fn_type(&[ptr.into(), ptr.into(), i64_ty.into()], false),
        );
        let root_count = roots
            .len()
            .checked_add(usize::from(result_root.is_some()))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or_else(|| CodegenError("caller-root count exceeds u64::MAX".to_string()))?;
        builder
            .build_call(
                push,
                &[
                    frame.into(),
                    entries_pointer.into(),
                    i64_ty.const_int(root_count, false).into(),
                ],
                "push_caller_roots",
            )
            .map_err(|error| CodegenError(format!("push caller roots: {error}")))?;

        let transition_ty = context.struct_type(
            &[
                ptr.into(),
                ptr.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                i64_ty.into(),
                context.i32_type().into(),
                context.i32_type().into(),
            ],
            false,
        );
        let transition = self.entry_alloca(transition_ty.into(), "native_transition")?;
        builder
            .build_store(transition, transition_ty.const_zero())
            .map_err(|error| CodegenError(format!("zero native transition: {error}")))?;
        let stack_pointer = builder
            .build_ptr_to_int(transition, i64_ty, "managed_stack_pointer")
            .map_err(|error| CodegenError(format!("managed stack pointer: {error}")))?;
        let enter_symbol = match kind {
            NativeTransitionKind::Safe => {
                scoop_lir::RuntimeAbiSymbolV1::EnterNativeSafe.logical_symbol()
            }
            NativeTransitionKind::Borrowed => {
                scoop_lir::RuntimeAbiSymbolV1::EnterNativeBorrowed.logical_symbol()
            }
        };
        let enter = self.native_boundary_fn(
            enter_symbol,
            context
                .void_type()
                .fn_type(&[ptr.into(), i64_ty.into()], false),
        );
        let enter_args = [transition.into(), stack_pointer.into()];
        statepoint::build_zero_live_call(
            context,
            self.llvm,
            builder,
            statepoint::ZeroLiveCall {
                callee: enter.as_global_value().as_pointer_value(),
                callee_type: enter.get_type(),
                call_args: &enter_args,
                safepoint,
                result_type: None,
            },
        )?;

        Ok(NativeTransition { frame, transition })
    }

    pub(in crate::function) fn leave_native_transition(
        &mut self,
        native: &NativeTransition<'ctx>,
        kind: NativeTransitionKind,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let leave_symbol = match kind {
            NativeTransitionKind::Safe => {
                scoop_lir::RuntimeAbiSymbolV1::LeaveNativeSafe.logical_symbol()
            }
            NativeTransitionKind::Borrowed => {
                scoop_lir::RuntimeAbiSymbolV1::LeaveNativeBorrowed.logical_symbol()
            }
        };
        let leave = self.native_boundary_fn(
            leave_symbol,
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(leave, &[native.transition.into()], "leave_native")
            .map_err(|error| CodegenError(format!("leave native transition: {error}")))?;

        Ok(())
    }

    pub(in crate::function) fn pop_native_roots(
        &mut self,
        native: NativeTransition<'ctx>,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let pop = self.native_boundary_fn(
            scoop_lir::RuntimeAbiSymbolV1::PopCallerRoots.logical_symbol(),
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(pop, &[native.frame.into()], "pop_caller_roots")
            .map_err(|error| CodegenError(format!("pop caller roots: {error}")))?;
        Ok(())
    }

    pub(in crate::function) fn finish_native_transition(
        &mut self,
        native: NativeTransition<'ctx>,
        kind: NativeTransitionKind,
        roots: &[scoop_lir::CallerRoot],
    ) -> Result<(), CodegenError> {
        self.leave_native_transition(&native, kind)?;
        let reloaded = self.reload_published_roots(roots.iter().map(|root| root.source))?;
        self.pop_native_roots(native)?;
        self.restore_reloaded_roots(reloaded)
    }
}
