use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Emit one typed direct/dispatch call. The protocol-specific LIR sum owns
    /// destination, safepoint/root plan and physical call independently; this
    /// routine receives one already-consistent arm and never infers an effect.
    pub(in crate::function) fn emit_typed_call(
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
    pub(in crate::function) fn emit_typed_invoke(
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
}
