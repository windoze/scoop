use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn emit_call_site(
        &mut self,
        site: &scoop_lir::CallSite,
    ) -> Result<(), CodegenError> {
        let targets = &self.function.call_targets;
        match site {
            scoop_lir::CallSite::Managed(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    scoop_lir::ManagedCallDestination::view,
                ),
                CallProtocol::Managed {
                    safepoint: site.safepoint,
                    live: &site.live,
                },
                None,
            ),
            scoop_lir::CallSite::NoGc(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    scoop_lir::NoGcCallDestination::view,
                ),
                CallProtocol::NoGc,
                None,
            ),
            scoop_lir::CallSite::NativeSafe(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.native_safe_targets,
                    scoop_lir::NativeSafeCallDestination::view,
                ),
                CallProtocol::NativeSafe {
                    safepoint: site.safepoint,
                    roots: &site.roots,
                },
                None,
            ),
            scoop_lir::CallSite::NativeBorrowed(site) => {
                let call = site.call.view(targets);
                self.emit_typed_call(
                    call.call,
                    CallProtocol::NativeBorrowed {
                        safepoint: site.safepoint,
                        roots: &site.roots,
                        result: call.result,
                    },
                    None,
                )
            }
        }
    }

    pub(super) fn emit_invoke_site(
        &mut self,
        site: &scoop_lir::InvokeSite,
    ) -> Result<(), CodegenError> {
        let targets = &self.function.call_targets;
        match site {
            scoop_lir::InvokeSite::Managed(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.managed_targets,
                    scoop_lir::ManagedCallDestination::view,
                ),
                CallProtocol::ManagedInvoke {
                    safepoint: site.safepoint,
                    roots: &site.roots,
                },
                Some((site.normal, site.unwind)),
            ),
            scoop_lir::InvokeSite::NoGc(site) => self.emit_typed_call(
                targets.typed_call_view(
                    &site.call,
                    &targets.no_gc_targets,
                    scoop_lir::NoGcCallDestination::view,
                ),
                CallProtocol::NoGc,
                Some((site.normal, site.unwind)),
            ),
        }
    }

    /// Emit one typed direct/dispatch call. The protocol-specific LIR sum owns
    /// destination, safepoint/root plan and physical call independently; this
    /// routine receives one already-consistent arm and never infers an effect.
    pub(super) fn emit_typed_call(
        &mut self,
        call: scoop_lir::TypedCallView<'_>,
        protocol: CallProtocol<'_>,
        invoke: Option<(scoop_lir::BlockId, scoop_lir::BlockId)>,
    ) -> Result<(), CodegenError> {
        let destination = call.destination();
        let (params, result) = match &call {
            scoop_lir::TypedCallView::Void { signature, .. } => {
                (signature.params.as_slice(), TypedCallResult::Void)
            }
            scoop_lir::TypedCallView::Direct { signature, out, .. } => (
                signature.params.as_slice(),
                TypedCallResult::Direct {
                    out: *out,
                    ty: &signature.result,
                    scan: &signature.result_scan,
                },
            ),
            scoop_lir::TypedCallView::IndirectResult {
                signature, storage, ..
            } => (
                signature.params.as_slice(),
                TypedCallResult::Indirect {
                    storage: *storage,
                    ty: &signature.result.ty,
                    scan: &signature.result.scan,
                },
            ),
        };

        let is_native = matches!(
            protocol,
            CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. }
        );
        if is_native && invoke.is_some() {
            return Err(CodegenError(format!(
                "typed call @{}: native calls cannot unwind through managed code",
                self.function.symbol
            )));
        }
        if call.args().len() != params.len() {
            return Err(CodegenError(format!(
                "typed call @{}: signature has {} parameters but call has {} arguments",
                self.function.symbol,
                params.len(),
                call.args().len()
            )));
        }

        match &result {
            TypedCallResult::Void => {}
            TypedCallResult::Direct { out, ty, .. } => {
                if &self.function.temps[*out].ty != *ty {
                    return Err(CodegenError(format!(
                        "typed call @{}: direct result temp does not match its signature",
                        self.function.symbol
                    )));
                }
            }
            TypedCallResult::Indirect { storage, ty, .. } => {
                if &self.function.locals[*storage].ty != *ty {
                    return Err(CodegenError(format!(
                        "typed call @{}: result storage does not match its signature",
                        self.function.symbol
                    )));
                }
            }
        }

        let managed_live = match &protocol {
            CallProtocol::Managed { safepoint, live } => {
                Some(self.materialize_statepoint_live(live, *safepoint)?)
            }
            CallProtocol::ManagedInvoke { .. }
            | CallProtocol::NoGc
            | CallProtocol::NativeSafe { .. }
            | CallProtocol::NativeBorrowed { .. } => None,
        };

        // Allocation is the one codegen-expanded runtime primitive. Its typed
        // identity selects the expansion; its symbol is not inspected.
        if destination
            == scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::Managed(
                scoop_lir::ManagedRuntimeFunction::Alloc,
            ))
        {
            let TypedCallResult::Direct { out, .. } = result else {
                return Err(CodegenError(format!(
                    "typed allocation @{} must have a direct result",
                    self.function.symbol
                )));
            };
            let CallProtocol::Managed { safepoint, live: _ } = &protocol else {
                return Err(CodegenError(format!(
                    "typed allocation @{} is not a managed call",
                    self.function.symbol
                )));
            };
            if invoke.is_some() {
                return Err(CodegenError(format!(
                    "typed allocation @{} cannot carry unwind edges",
                    self.function.symbol
                )));
            }
            self.managed_alloc(
                out,
                call.args(),
                *safepoint,
                managed_live.expect("managed allocation prepared its live set"),
            )?;
            return Ok(());
        }

        let mut param_tys = params
            .iter()
            .map(|ty| {
                basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    ty,
                )
                .map(Into::into)
            })
            .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
        let fn_ty = match &result {
            TypedCallResult::Void => self.context.void_type().fn_type(&param_tys, false),
            TypedCallResult::Direct { ty, .. } => basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                ty,
            )?
            .fn_type(&param_tys, false),
            TypedCallResult::Indirect { .. } => {
                param_tys.insert(0, ptr_ty(self.context).into());
                self.context.void_type().fn_type(&param_tys, false)
            }
        };

        let mut call_args = call
            .args()
            .iter()
            .map(|argument| match managed_live.as_ref() {
                Some(live) => self.statepoint_value(*argument, live),
                None => self.value(*argument),
            })
            .collect::<Result<Vec<BasicValueEnum<'ctx>>, _>>()?;
        if let TypedCallResult::Indirect { storage, .. } = &result {
            call_args.insert(0, self.allocas[arena_index(*storage)].into());
        }

        let native = match &protocol {
            CallProtocol::NativeSafe { roots, .. } => {
                if result_scan(&result) != &RefScan::None {
                    return Err(CodegenError(format!(
                        "native-safe call @{} has a managed result",
                        self.function.symbol
                    )));
                }
                Some((NativeTransitionKind::Safe, roots.as_slice(), None))
            }
            CallProtocol::NativeBorrowed {
                roots,
                result: publication,
                ..
            } => {
                let result_root = match *publication {
                    scoop_lir::NativeBorrowedResultPublication::DirectRooted { storage, scan }
                    | scoop_lir::NativeBorrowedResultPublication::IndirectResultRooted {
                        storage,
                        scan,
                    } => {
                        let llvm_ty = basic_ty(
                            self.context,
                            self.structs,
                            self.enums,
                            self.managed_address_space,
                            &self.function.locals[storage].ty,
                        )?;
                        let pointer = self.allocas[arena_index(storage)];
                        self.builder
                            .build_store(pointer, llvm_ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero native result: {error}"))
                            })?;
                        Some((pointer, llvm_ty, scan.as_ref_scan()))
                    }
                    scoop_lir::NativeBorrowedResultPublication::Void
                    | scoop_lir::NativeBorrowedResultPublication::DirectGcFree
                    | scoop_lir::NativeBorrowedResultPublication::IndirectResultGcFree => None,
                };
                Some((
                    NativeTransitionKind::Borrowed,
                    roots.as_slice(),
                    result_root,
                ))
            }
            CallProtocol::Managed { .. } => None,
            CallProtocol::ManagedInvoke { roots, .. } => {
                let _ = roots;
                None
            }
            CallProtocol::NoGc => None,
        };
        let native_result_storage = native.and_then(|(_, _, result)| result);
        let transition = if let Some((kind, roots, _)) = native {
            let safepoint = protocol
                .safepoint()
                .expect("native protocol always carries a safepoint id");
            self.publish_native_roots(
                roots,
                native_result_storage.map(|(storage, _, scan)| (storage, scan)),
                kind,
                safepoint,
            )
            .map(Some)?
        } else {
            None
        };

        if native.is_some() {
            // Entering either native transition may park this thread. The
            // collector then updates the published canonical storage, while
            // any SSA arguments evaluated before the transition retain their
            // old addresses. Rebuild the physical call arguments after the
            // handshake so direct refs and managed leaves inside aggregates
            // are loaded from the relocated caller-root storage.
            call_args = call
                .args()
                .iter()
                .map(|argument| self.value(*argument))
                .collect::<Result<Vec<_>, _>>()?;
            if let TypedCallResult::Indirect { storage, .. } = &result {
                call_args.insert(0, self.allocas[arena_index(*storage)].into());
            }
        }

        let callee = match destination {
            scoop_lir::CallDestination::Dispatch { .. } => None,
            _ => Some(self.typed_callee(destination, fn_ty)?),
        };
        if let Some((normal, unwind)) = invoke {
            return self.emit_typed_invoke(
                destination,
                fn_ty,
                &call_args,
                &result,
                &protocol,
                callee,
                normal,
                unwind,
            );
        }
        let direct_value = if native.is_some() {
            let actual_callee = match destination {
                scoop_lir::CallDestination::Dispatch { table, slot } => {
                    self.dispatch_function_pointer(table, slot)?
                }
                _ => callee
                    .expect("direct destination has a typed callee")
                    .as_global_value()
                    .as_pointer_value(),
            };
            let arguments = call_args
                .iter()
                .copied()
                .map(Into::into)
                .collect::<Vec<_>>();
            let call = self
                .builder
                .build_indirect_call(fn_ty, actual_callee, &arguments, "native_call")
                .map_err(|error| CodegenError(format!("typed native call: {error}")))?;
            self.apply_nounwind(call);
            call.add_attribute(
                AttributeLoc::Function,
                self.context.create_string_attribute("gc-leaf-function", ""),
            );
            match &result {
                TypedCallResult::Direct { .. } => match call.try_as_basic_value() {
                    ValueKind::Basic(value) => Some(value),
                    ValueKind::Instruction(_) => {
                        return Err(CodegenError(format!(
                            "typed native call @{} produced no direct value",
                            self.function.symbol
                        )));
                    }
                },
                TypedCallResult::Void | TypedCallResult::Indirect { .. } => None,
            }
        } else {
            let call = match destination {
                scoop_lir::CallDestination::Dispatch { table, slot } => {
                    let pointer = self.dispatch_function_pointer(table, slot)?;
                    let arguments = call_args
                        .iter()
                        .copied()
                        .map(Into::into)
                        .collect::<Vec<_>>();
                    self.builder
                        .build_indirect_call(fn_ty, pointer, &arguments, "typed_call")
                        .map_err(|error| CodegenError(format!("typed dispatch call: {error}")))?
                }
                _ => {
                    let arguments = call_args
                        .iter()
                        .copied()
                        .map(Into::into)
                        .collect::<Vec<_>>();
                    self.builder
                        .build_call(
                            callee.expect("direct destination has a callee"),
                            &arguments,
                            "typed_call",
                        )
                        .map_err(|error| CodegenError(format!("typed direct call: {error}")))?
                }
            };
            self.apply_call_protocol(call, destination, &protocol);
            match &result {
                TypedCallResult::Direct { .. } => match call.try_as_basic_value() {
                    ValueKind::Basic(value) => Some(value),
                    ValueKind::Instruction(_) => {
                        return Err(CodegenError(format!(
                            "typed call @{} produced no direct value",
                            self.function.symbol
                        )));
                    }
                },
                TypedCallResult::Void | TypedCallResult::Indirect { .. } => None,
            }
        };

        if let (Some(live), CallProtocol::Managed { safepoint, .. }) = (managed_live, &protocol) {
            self.restore_statepoint_live(live, *safepoint)?;
        }

        if let (Some(value), TypedCallResult::Direct { .. }, Some((storage, _, _))) =
            (direct_value, &result, native_result_storage)
        {
            self.builder
                .build_store(storage, value)
                .map_err(|error| CodegenError(format!("store native result: {error}")))?;
        }
        if let Some(transition) = transition {
            let (kind, _, _) = native.expect("a native transition has its typed protocol");
            self.finish_native_transition(transition, kind)?;
        }

        if let TypedCallResult::Direct { out, .. } = result {
            let value = if let Some((storage, ty, _)) = native_result_storage {
                self.builder
                    .build_load(ty, storage, "native_result")
                    .map_err(|error| CodegenError(format!("load native result: {error}")))?
            } else {
                direct_value.expect("direct typed call produced a value")
            };
            self.temps.insert(out, value);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_typed_invoke(
        &mut self,
        destination: scoop_lir::CallDestination,
        fn_type: inkwell::types::FunctionType<'ctx>,
        call_args: &[BasicValueEnum<'ctx>],
        result: &TypedCallResult<'_>,
        protocol: &CallProtocol<'_>,
        callee: Option<inkwell::values::FunctionValue<'ctx>>,
        normal: scoop_lir::BlockId,
        unwind: scoop_lir::BlockId,
    ) -> Result<(), CodegenError> {
        let roots = match protocol {
            CallProtocol::ManagedInvoke { roots, .. } => roots.as_slice(),
            CallProtocol::NoGc => &[],
            CallProtocol::Managed { .. }
            | CallProtocol::NativeSafe { .. }
            | CallProtocol::NativeBorrowed { .. } => {
                return Err(CodegenError(format!(
                    "non-invoke protocol reached invoke emission in @{}",
                    self.function.symbol
                )));
            }
        };
        let frame = self.publish_compiler_roots(roots)?;
        let cleanup = self.context.append_basic_block(
            self.llvm_function,
            &format!("invoke.normal.cleanup.{}", self.compiler_invoke_index - 1),
        );
        let actual_callee = match destination {
            scoop_lir::CallDestination::Dispatch { table, slot } => {
                self.dispatch_function_pointer(table, slot)?
            }
            _ => callee
                .expect("direct destination has a typed callee")
                .as_global_value()
                .as_pointer_value(),
        };
        let result_type = match result {
            TypedCallResult::Direct { ty, .. } => Some(basic_ty(
                self.context,
                self.structs,
                self.enums,
                self.managed_address_space,
                ty,
            )?),
            TypedCallResult::Void | TypedCallResult::Indirect { .. } => None,
        };
        let direct_value = match protocol {
            CallProtocol::ManagedInvoke { safepoint, .. } => statepoint::build_managed_invoke(
                self.context,
                self.llvm,
                self.builder,
                statepoint::ManagedInvoke {
                    callee: actual_callee,
                    callee_type: fn_type,
                    call_args,
                    safepoint: *safepoint,
                    normal: cleanup,
                    unwind: self.llvm_blocks[arena_index(unwind)],
                    result_type,
                },
            )?,
            CallProtocol::NoGc => {
                let call = match destination {
                    scoop_lir::CallDestination::Dispatch { .. } => self
                        .builder
                        .build_indirect_invoke(
                            fn_type,
                            actual_callee,
                            call_args,
                            cleanup,
                            self.llvm_blocks[arena_index(unwind)],
                            "typed_invoke",
                        )
                        .map_err(|error| CodegenError(format!("typed dispatch invoke: {error}")))?,
                    _ => self
                        .builder
                        .build_invoke(
                            callee.expect("direct destination has a typed callee"),
                            call_args,
                            cleanup,
                            self.llvm_blocks[arena_index(unwind)],
                            "typed_invoke",
                        )
                        .map_err(|error| CodegenError(format!("typed direct invoke: {error}")))?,
                };
                self.apply_call_protocol(call, destination, protocol);
                self.builder.position_at_end(cleanup);
                match result {
                    TypedCallResult::Direct { .. } => call.try_as_basic_value().basic(),
                    TypedCallResult::Void | TypedCallResult::Indirect { .. } => None,
                }
            }
            _ => unreachable!("protocol was checked above"),
        };

        self.validate_compiler_root_sources(
            roots
                .iter()
                .filter(|root| root.normal_live)
                .map(|root| root.root.source),
        )?;
        self.pop_compiler_roots(frame)?;
        if let TypedCallResult::Direct { out, .. } = result {
            let value = direct_value.ok_or_else(|| {
                CodegenError(format!(
                    "typed invoke @{} produced no direct result",
                    self.function.symbol
                ))
            })?;
            self.temps.insert(*out, value);
            self.sync_root_temp(*out)?;
        }
        self.builder
            .build_unconditional_branch(self.llvm_blocks[arena_index(normal)])
            .map_err(|error| CodegenError(format!("leave invoke cleanup: {error}")))?;
        Ok(())
    }

    pub(super) fn typed_callee(
        &self,
        destination: scoop_lir::CallDestination,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, CodegenError> {
        let symbol = match destination {
            scoop_lir::CallDestination::Local(id) => {
                let symbol = self
                    .functions
                    .get(id.into_u32() as usize)
                    .ok_or_else(|| {
                        CodegenError(format!("invalid local function id {}", id.into_u32()))
                    })?
                    .symbol
                    .as_str();
                let function = self.llvm.get_function(symbol).ok_or_else(|| {
                    CodegenError(format!(
                        "typed local target `{symbol}` was not declared in the module pass"
                    ))
                })?;
                if function.get_type() != fn_ty {
                    return Err(CodegenError(format!(
                        "typed target `{symbol}` disagrees with its existing declaration"
                    )));
                }
                return Ok(function);
            }
            scoop_lir::CallDestination::Runtime(function) => function.symbol(),
            scoop_lir::CallDestination::Extern(id) => match &self.extern_functions[id].kind {
                ExternFunctionKind::C { bridge_symbol, .. } => bridge_symbol,
                ExternFunctionKind::Scoop { .. } => &self.extern_functions[id].native_symbol,
            },
            scoop_lir::CallDestination::Dispatch { .. } => {
                unreachable!("dispatch destinations have no direct callee")
            }
        };
        if let Some(function) = self.llvm.get_function(symbol) {
            if function.get_type() != fn_ty {
                return Err(CodegenError(format!(
                    "typed target `{symbol}` disagrees with its existing declaration"
                )));
            }
            Ok(function)
        } else {
            Ok(self.llvm.add_function(symbol, fn_ty, None))
        }
    }

    pub(super) fn dispatch_function_pointer(
        &self,
        table: Value,
        slot: scoop_lir::DispatchSlotId,
    ) -> Result<PointerValue<'ctx>, CodegenError> {
        let table = self.value(table)?.into_pointer_value();
        let slot = self.function.call_targets.dispatch_slots[slot];
        // SAFETY: the typed dispatch slot is assigned by lir-lower from the
        // complete vtable/itable/closure layout.
        let slot_pointer = unsafe {
            self.builder.build_gep(
                ptr_ty(self.context),
                table,
                &[self.context.i32_type().const_int(slot.index.into(), false)],
                "dispatch_slot",
            )
        }
        .map_err(|error| CodegenError(format!("typed dispatch slot: {error}")))?;
        self.builder
            .build_load(ptr_ty(self.context), slot_pointer, "dispatch_function")
            .map(BasicValueEnum::into_pointer_value)
            .map_err(|error| CodegenError(format!("typed dispatch load: {error}")))
    }

    pub(super) fn apply_call_protocol(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        destination: scoop_lir::CallDestination,
        protocol: &CallProtocol<'_>,
    ) {
        if matches!(protocol, CallProtocol::NoGc) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context.create_string_attribute("gc-leaf-function", ""),
            );
        }
        if matches!(
            protocol,
            CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. }
        ) {
            self.apply_nounwind(call);
        }
        if let Some(safepoint) = protocol.safepoint() {
            self.apply_safepoint_id(call, safepoint);
        }
        if matches!(
            destination,
            scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(
                scoop_lir::NoGcRuntimeFunction::Trap
                    | scoop_lir::NoGcRuntimeFunction::Throw
                    | scoop_lir::NoGcRuntimeFunction::Rethrow
            ))
        ) {
            call.add_attribute(
                AttributeLoc::Function,
                self.context
                    .create_enum_attribute(Attribute::get_named_enum_kind_id("noreturn"), 0),
            );
        }
    }

    pub(super) fn apply_nounwind(&self, call: inkwell::values::CallSiteValue<'ctx>) {
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
        );
    }

    pub(super) fn apply_safepoint_id(
        &self,
        call: inkwell::values::CallSiteValue<'ctx>,
        safepoint: scoop_lir::SafepointId,
    ) {
        call.add_attribute(
            AttributeLoc::Function,
            self.context
                .create_string_attribute("statepoint-id", &safepoint.get().to_string()),
        );
    }

    pub(super) fn native_boundary_fn(
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

    pub(super) fn publish_native_roots(
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
        let call_index = self.native_call_index;
        self.native_call_index += 1;

        let mut entries = Vec::with_capacity(roots.len() + usize::from(result_root.is_some()));
        for (index, root) in roots.iter().enumerate() {
            let storage = self.root_source_storage(root.source)?;
            let descriptor = emit_ref_scan(
                context,
                self.llvm,
                &format!("{}.native.{call_index}.root.{index}", self.function.symbol),
                root.scan.as_ref_scan(),
            )
            .expect("LIR caller roots always carry a non-empty scan");
            entries.push((storage.pointer, descriptor));
        }
        if let Some((storage, scan)) = result_root {
            let descriptor = emit_ref_scan(
                context,
                self.llvm,
                &format!("{}.native.{call_index}.result", self.function.symbol),
                scan,
            )
            .expect("a native result root always carries a non-empty scan");
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
            "scoop_rt_push_caller_roots",
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
            NativeTransitionKind::Safe => "scoop_rt_enter_native_safe",
            NativeTransitionKind::Borrowed => "scoop_rt_enter_native_borrowed",
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

    pub(super) fn finish_native_transition(
        &mut self,
        native: NativeTransition<'ctx>,
        kind: NativeTransitionKind,
    ) -> Result<(), CodegenError> {
        let context = self.context;
        let ptr = ptr_ty(context);
        let leave_symbol = match kind {
            NativeTransitionKind::Safe => "scoop_rt_leave_native_safe",
            NativeTransitionKind::Borrowed => "scoop_rt_leave_native_borrowed",
        };
        let leave = self.native_boundary_fn(
            leave_symbol,
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(leave, &[native.transition.into()], "leave_native")
            .map_err(|error| CodegenError(format!("leave native transition: {error}")))?;

        let pop = self.native_boundary_fn(
            "scoop_rt_pop_caller_roots",
            context.void_type().fn_type(&[ptr.into()], false),
        );
        self.builder
            .build_call(pop, &[native.frame.into()], "pop_caller_roots")
            .map_err(|error| CodegenError(format!("pop caller roots: {error}")))?;
        Ok(())
    }
}
