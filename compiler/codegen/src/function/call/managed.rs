use super::*;

mod invoke;
mod validation;

impl<'ctx> FnEmitter<'_, 'ctx> {
    fn physical_call_arguments(
        &self,
        arguments: &[scoop_lir::AbiCallArgument],
        result: &TypedCallResult<'_>,
        live: Option<&MaterializedStatepointLive<'ctx>>,
    ) -> Result<Vec<BasicValueEnum<'ctx>>, CodegenError> {
        let mut values = Vec::with_capacity(
            arguments.len() + usize::from(matches!(result, TypedCallResult::Indirect { .. })),
        );
        if let TypedCallResult::Indirect { storage, .. } = result {
            values.push(self.local_pointer(*storage)?.into());
        }
        for argument in arguments {
            match *argument {
                scoop_lir::AbiCallArgument::ElidedZst(_) => {}
                scoop_lir::AbiCallArgument::Direct(value) => {
                    values.push(self.typed_call_argument_value(value, live)?);
                }
                scoop_lir::AbiCallArgument::Indirect(storage) => {
                    values.push(self.local_pointer(storage.local())?.into());
                }
            }
        }
        Ok(values)
    }

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
        let (signature, result) = match &call {
            scoop_lir::TypedCallView::Void { signature, .. } => (
                scoop_lir::ScoopAbiSignature::new(
                    signature.arguments().to_vec(),
                    scoop_lir::AbiReturn::UnitVoid,
                    signature.calling_convention(),
                ),
                TypedCallResult::Void,
            ),
            scoop_lir::TypedCallView::ElidedZst { signature, out, .. } => (
                scoop_lir::ScoopAbiSignature::new(
                    signature.arguments().to_vec(),
                    scoop_lir::AbiReturn::ElidedZst(signature.result().clone()),
                    signature.calling_convention(),
                ),
                TypedCallResult::ElidedZst {
                    out: *out,
                    value: signature.result(),
                },
            ),
            scoop_lir::TypedCallView::Direct { signature, out, .. } => (
                scoop_lir::ScoopAbiSignature::new(
                    signature.arguments().to_vec(),
                    scoop_lir::AbiReturn::Direct(signature.result().clone()),
                    signature.calling_convention(),
                ),
                TypedCallResult::Direct {
                    out: *out,
                    value: signature.result(),
                },
            ),
            scoop_lir::TypedCallView::IndirectResult {
                signature, storage, ..
            } => (
                scoop_lir::ScoopAbiSignature::new(
                    signature.arguments().to_vec(),
                    scoop_lir::AbiReturn::Indirect(signature.result().clone()),
                    signature.calling_convention(),
                ),
                TypedCallResult::Indirect {
                    storage: *storage,
                    value: signature.result(),
                    convention: signature.convention(),
                },
            ),
        };
        self.validate_typed_call_contract(
            &call,
            destination,
            &signature,
            &result,
            &protocol,
            invoke.is_some(),
        )?;

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
                    self.function.symbol()
                )));
            };
            let CallProtocol::Managed { safepoint, live: _ } = &protocol else {
                return Err(CodegenError(format!(
                    "typed allocation @{} is not a managed call",
                    self.function.symbol()
                )));
            };
            if invoke.is_some() {
                return Err(CodegenError(format!(
                    "typed allocation @{} cannot carry unwind edges",
                    self.function.symbol()
                )));
            }
            let logical_args = call
                .args()
                .iter()
                .map(|argument| argument.logical_value())
                .collect::<Vec<_>>();
            self.managed_alloc(
                out,
                &logical_args,
                *safepoint,
                managed_live.expect("managed allocation prepared its live set"),
            )?;
            return Ok(());
        }

        let fn_ty = abi::function_type(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            &signature,
        )?;
        let mut call_args =
            self.physical_call_arguments(call.args(), &result, managed_live.as_ref())?;

        let native = match &protocol {
            CallProtocol::NativeSafe { roots, .. } => {
                if result_scan(&result) != &RefScan::None {
                    return Err(CodegenError(format!(
                        "native-safe call @{} has a managed result",
                        self.function.symbol()
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
                            self.function.locals[storage].ty(),
                        )?;
                        let pointer = self.local_pointer(storage)?;
                        self.builder
                            .build_store(pointer, llvm_ty.const_zero())
                            .map_err(|error| {
                                CodegenError(format!("zero native result: {error}"))
                            })?;
                        Some((pointer, llvm_ty, scan.as_ref_scan()))
                    }
                    scoop_lir::NativeBorrowedResultPublication::Void
                    | scoop_lir::NativeBorrowedResultPublication::ElidedZst
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
            call_args = self.physical_call_arguments(call.args(), &result, None)?;
        }

        let apply_scoop_abi_attributes = match destination {
            scoop_lir::CallDestination::Local(_)
            | scoop_lir::CallDestination::External(_)
            | scoop_lir::CallDestination::Dispatch { .. } => true,
            scoop_lir::CallDestination::Extern(id) => matches!(
                self.extern_functions[id].kind,
                ExternFunctionKind::Scoop { .. }
            ),
            scoop_lir::CallDestination::Runtime(_) => false,
        };
        let callee = match destination {
            scoop_lir::CallDestination::Dispatch { .. } => None,
            _ => Some(self.typed_callee(
                destination,
                fn_ty,
                &signature,
                apply_scoop_abi_attributes,
            )?),
        };
        if let Some((normal, unwind)) = invoke {
            return self.emit_typed_invoke(
                destination,
                fn_ty,
                &call_args,
                &result,
                &signature,
                apply_scoop_abi_attributes,
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
            if apply_scoop_abi_attributes {
                abi::apply_call_attributes(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    call,
                    &signature,
                )?;
            }
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
                            self.function.symbol()
                        )));
                    }
                },
                TypedCallResult::Void
                | TypedCallResult::ElidedZst { .. }
                | TypedCallResult::Indirect { .. } => None,
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
            if apply_scoop_abi_attributes {
                abi::apply_call_attributes(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    call,
                    &signature,
                )?;
            }
            self.apply_call_protocol(call, destination, &protocol);
            match &result {
                TypedCallResult::Direct { .. } => match call.try_as_basic_value() {
                    ValueKind::Basic(value) => Some(value),
                    ValueKind::Instruction(_) => {
                        return Err(CodegenError(format!(
                            "typed call @{} produced no direct value",
                            self.function.symbol()
                        )));
                    }
                },
                TypedCallResult::Void
                | TypedCallResult::ElidedZst { .. }
                | TypedCallResult::Indirect { .. } => None,
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
        let mut rooted_native_result = None;
        if let Some(transition) = transition {
            let (kind, roots, _) = native.expect("a native transition has its typed protocol");
            self.leave_native_transition(&transition, kind)?;
            let reloaded_roots =
                self.reload_published_roots(roots.iter().map(|root| root.source))?;
            if let Some((storage, ty, _)) = native_result_storage {
                rooted_native_result = Some(
                    self.builder
                        .build_load(ty, storage, "native_result_reload")
                        .map_err(|error| {
                            CodegenError(format!("reload rooted native result: {error}"))
                        })?,
                );
            }
            self.pop_native_roots(transition)?;
            self.restore_reloaded_roots(reloaded_roots)?;
        }

        match result {
            TypedCallResult::ElidedZst { out, value } => {
                let ty = basic_ty(
                    self.context,
                    self.structs,
                    self.enums,
                    self.managed_address_space,
                    value.storage_type(),
                )?;
                self.temps.insert(out, ty.const_zero());
            }
            TypedCallResult::Direct { out, .. } => {
                let value = rooted_native_result
                    .unwrap_or_else(|| direct_value.expect("direct typed call produced a value"));
                self.temps.insert(out, value);
            }
            TypedCallResult::Indirect { storage, .. } => {
                if let Some(value) = rooted_native_result {
                    self.builder
                        .build_store(self.local_pointer(storage)?, value)
                        .map_err(|error| {
                            CodegenError(format!("preserve reloaded native result: {error}"))
                        })?;
                }
            }
            TypedCallResult::Void => {}
        }
        Ok(())
    }
}
