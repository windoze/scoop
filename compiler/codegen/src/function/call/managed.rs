use super::*;

mod arguments;
mod invoke;
mod preparation;
mod validation;

use preparation::CallPreparation;

impl<'ctx> FnEmitter<'_, 'ctx> {
    /// Emit one typed direct/dispatch call. The protocol-specific LIR sum owns
    /// destination, safepoint/root plan and physical call together; this
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

        if invoke.is_none()
            && let (
                scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(
                    operation @ (scoop_lir::NoGcRuntimeFunction::StringByteLength
                    | scoop_lir::NoGcRuntimeFunction::StringByteAt),
                )),
                TypedCallResult::Direct { out, .. },
            ) = (destination, &result)
        {
            let object = self
                .value(call.args()[0].logical_value())?
                .into_pointer_value();
            let length = self.string_byte_length(object)?;
            let value = if operation == scoop_lir::NoGcRuntimeFunction::StringByteAt {
                let index = self.value(call.args()[1].logical_value())?.into_int_value();
                self.string_byte_at(object, index, length)?
            } else {
                length
            };
            self.temps.insert(*out, value.into());
            return Ok(());
        }

        if self.optimize_allocations
            && invoke.is_none()
            && let (
                scoop_lir::CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(
                    scoop_lir::NoGcRuntimeFunction::StringCompare,
                )),
                TypedCallResult::Direct { out, .. },
            ) = (destination, &result)
            && let Some(value) = self.short_string_compare(
                call.args()[0].logical_value(),
                call.args()[1].logical_value(),
            )?
        {
            self.temps.insert(*out, value.into());
            return Ok(());
        }

        // Typed runtime identities select expansions; symbols are not inspected.
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
            let CallProtocol::Managed { safepoint, live } = &protocol else {
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
            let live = self.materialize_statepoint_live(live, *safepoint)?;
            self.managed_alloc(out, &logical_args, *safepoint, live)?;
            return Ok(());
        }

        let fn_ty = abi::function_type(
            self.context,
            self.structs,
            self.enums,
            self.managed_address_space,
            &signature,
        )?;

        let preparation = self.prepare_call_protocol(&protocol, &result)?;

        // A native handshake may relocate the published roots. Materialize
        // arguments afterwards, and coerce each GC-free aggregate only once.
        let call_args = self.physical_call_arguments(
            destination,
            call.args(),
            &signature,
            &result,
            preparation.managed_live(),
        )?;

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
        let callee =
            self.prepare_callee(destination, fn_ty, &signature, apply_scoop_abi_attributes)?;
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
        let direct_value = if matches!(&preparation, CallPreparation::Native(_)) {
            let actual_callee = self.prepared_callee_pointer(callee)?;
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
            self.apply_c_abi_attributes(destination, |location, attribute| {
                call.add_attribute(location, attribute)
            })?;
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
            let call = match callee {
                PreparedCallee::Dispatch { table, slot } => {
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
                PreparedCallee::Direct(function) => {
                    let arguments = call_args
                        .iter()
                        .copied()
                        .map(Into::into)
                        .collect::<Vec<_>>();
                    self.builder
                        .build_call(function, &arguments, "typed_call")
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
            self.apply_c_abi_attributes(destination, |location, attribute| {
                call.add_attribute(location, attribute)
            })?;
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

        let direct_value = match (&result, direct_value) {
            (TypedCallResult::Direct { value: plan, .. }, Some(value)) => {
                Some(self.decode_direct_result(plan, value)?)
            }
            (_, value) => value,
        };

        let rooted_native_result = match preparation {
            CallPreparation::Managed { live, safepoint } => {
                self.restore_statepoint_live(live, safepoint)?;
                None
            }
            CallPreparation::Native(native) => {
                self.finish_prepared_native(native, &result, direct_value)?
            }
            CallPreparation::Ordinary => None,
        };

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
                let value = rooted_native_result.or(direct_value).ok_or_else(|| {
                    CodegenError(format!(
                        "typed call @{} produced no direct value",
                        self.function.symbol()
                    ))
                })?;
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
