use super::*;

impl<'ctx> FnEmitter<'_, 'ctx> {
    pub(super) fn validate_typed_call_contract(
        &self,
        call: &scoop_lir::TypedCallView<'_>,
        destination: scoop_lir::CallDestination,
        signature: &scoop_lir::ScoopAbiSignature,
        result: &TypedCallResult<'_>,
        protocol: &CallProtocol<'_>,
        has_invoke: bool,
    ) -> Result<(), CodegenError> {
        let params = signature.arguments();

        let is_native = matches!(
            protocol,
            CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. }
        );
        if is_native && has_invoke {
            return Err(CodegenError(format!(
                "typed call @{}: native calls cannot unwind through managed code",
                self.function.symbol()
            )));
        }
        if call.args().len() != params.len() {
            return Err(CodegenError(format!(
                "typed call @{}: signature has {} parameters but call has {} arguments",
                self.function.symbol(),
                params.len(),
                call.args().len()
            )));
        }
        for (index, (argument, parameter)) in call.args().iter().zip(params).enumerate() {
            let convention_matches = matches!(
                (argument, parameter),
                (
                    scoop_lir::AbiCallArgument::ElidedZst(_),
                    scoop_lir::AbiArgument::ElidedZst(_)
                ) | (
                    scoop_lir::AbiCallArgument::Direct(_),
                    scoop_lir::AbiArgument::Direct(_)
                ) | (
                    scoop_lir::AbiCallArgument::Indirect(_),
                    scoop_lir::AbiArgument::Indirect(_)
                )
            );
            let argument_ty = self
                .function
                .value_ty(self.globals_arena, argument.logical_value());
            if !convention_matches || &argument_ty != parameter.logical_storage_type() {
                return Err(CodegenError(format!(
                    "typed call @{}: argument {} convention/type does not match {}",
                    self.function.symbol(),
                    index,
                    parameter.logical_storage_type().dump()
                )));
            }
        }

        let carries_c_argument_storage = call.args().iter().any(|argument| {
            matches!(
                argument,
                scoop_lir::AbiCallArgument::Direct(scoop_lir::Value::CArgumentStorage(_))
            )
        });
        let is_c_extern = match destination {
            scoop_lir::CallDestination::Extern(id) => matches!(
                &self.extern_functions[id].kind,
                ExternFunctionKind::C { .. }
            ),
            scoop_lir::CallDestination::Local(_)
            | scoop_lir::CallDestination::External(_)
            | scoop_lir::CallDestination::Runtime(_)
            | scoop_lir::CallDestination::Dispatch { .. } => false,
        };
        if carries_c_argument_storage && !is_c_extern {
            return Err(CodegenError(format!(
                "typed call @{} uses a C argument-storage address outside a C extern call",
                self.function.symbol()
            )));
        }

        match destination {
            scoop_lir::CallDestination::Local(id) => {
                let declaration = self.functions.get(id.into_u32() as usize).ok_or_else(|| {
                    CodegenError(format!(
                        "typed local call @{} has invalid function id {}",
                        self.function.symbol(),
                        id.into_u32()
                    ))
                })?;
                if signature != &declaration.signature
                    || matches!(
                        result,
                        TypedCallResult::Indirect {
                            convention: scoop_lir::IndirectResultConvention::CStoragePointer,
                            ..
                        }
                    )
                {
                    return Err(CodegenError(format!(
                        "typed local call @{} ABI does not match `{}`",
                        self.function.symbol(),
                        declaration.symbol()
                    )));
                }
                let expected_effect = match protocol {
                    CallProtocol::Managed { .. } | CallProtocol::ManagedInvoke { .. } => {
                        scoop_lir::GcEffect::Managed
                    }
                    CallProtocol::NoGc => scoop_lir::GcEffect::NoGc,
                    CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. } => {
                        return Err(CodegenError(format!(
                            "typed local call @{} cannot use a native transition protocol",
                            self.function.symbol()
                        )));
                    }
                };
                if declaration.gc_effect != expected_effect {
                    return Err(CodegenError(format!(
                        "typed local call @{} protocol does not match the GC effect of `{}`",
                        self.function.symbol(),
                        declaration.symbol()
                    )));
                }
            }
            scoop_lir::CallDestination::External(id) => {
                let declaration = &self.external_callables[id];
                if signature != declaration.signature()
                    || matches!(
                        result,
                        TypedCallResult::Indirect {
                            convention: scoop_lir::IndirectResultConvention::CStoragePointer,
                            ..
                        }
                    )
                {
                    return Err(CodegenError(format!(
                        "typed external call @{} ABI does not match `{}`",
                        self.function.symbol(),
                        declaration.expected_symbol().symbol()
                    )));
                }
                let expected_effect = match protocol {
                    CallProtocol::Managed { .. } | CallProtocol::ManagedInvoke { .. } => {
                        scoop_lir::GcEffect::Managed
                    }
                    CallProtocol::NoGc => scoop_lir::GcEffect::NoGc,
                    CallProtocol::NativeSafe { .. } | CallProtocol::NativeBorrowed { .. } => {
                        return Err(CodegenError(format!(
                            "typed external call @{} cannot use a native transition protocol",
                            self.function.symbol()
                        )));
                    }
                };
                if declaration.gc_effect() != expected_effect {
                    return Err(CodegenError(format!(
                        "typed external call @{} protocol does not match the GC effect of `{}`",
                        self.function.symbol(),
                        declaration.expected_symbol().symbol()
                    )));
                }
            }
            scoop_lir::CallDestination::Extern(id) => {
                let declaration = &self.extern_functions[id];
                let (params_match, result_matches, protocol_matches) = match &declaration.kind {
                    ExternFunctionKind::C {
                        signature: c_signature,
                        ..
                    } => {
                        let return_type = c_signature.storage_return_type();
                        let is_void = c_signature.return_type.is_void();
                        if call.args().len() == c_signature.params.len() {
                            for (index, (argument, parameter)) in
                                call.args().iter().zip(&c_signature.params).enumerate()
                            {
                                let scoop_lir::AbiCallArgument::Direct(
                                    scoop_lir::Value::CArgumentStorage(storage),
                                ) = argument
                                else {
                                    return Err(CodegenError(format!(
                                        "typed C extern call @{}: argument {} is not an exact C argument-storage address",
                                        self.function.symbol(),
                                        index
                                    )));
                                };
                                let local_index = arena_index(storage.local());
                                if local_index >= self.function.locals.len() {
                                    return Err(CodegenError(format!(
                                        "typed C extern call @{}: argument {} refers to invalid storage local{}",
                                        self.function.symbol(),
                                        index,
                                        local_index
                                    )));
                                }
                                let actual = self.function.locals[storage.local()].ty();
                                let expected = parameter.storage_type();
                                if actual != &expected {
                                    return Err(CodegenError(format!(
                                        "typed C extern call @{}: argument {} storage local{} has type {}, expected exact C storage {}",
                                        self.function.symbol(),
                                        index,
                                        local_index,
                                        actual.dump(),
                                        expected.dump()
                                    )));
                                }
                            }
                        }
                        (
                            params.len() == c_signature.params.len()
                                && params.iter().all(|argument| {
                                    matches!(
                                        argument,
                                        scoop_lir::AbiArgument::Direct(value)
                                            if value.storage_type() == &scoop_lir::RAW_PTR
                                    )
                                }),
                            match result {
                                TypedCallResult::Void => is_void,
                                TypedCallResult::Indirect {
                                    value,
                                    convention: scoop_lir::IndirectResultConvention::CStoragePointer,
                                    ..
                                } => !is_void && value.storage_type() == &return_type,
                                TypedCallResult::ElidedZst { .. }
                                | TypedCallResult::Direct { .. }
                                | TypedCallResult::Indirect { .. } => false,
                            },
                            matches!(protocol, CallProtocol::NativeSafe { .. }),
                        )
                    }
                    ExternFunctionKind::Scoop {
                        signature: declaration_signature,
                        ..
                    } => (
                        signature == declaration_signature,
                        !matches!(
                            result,
                            TypedCallResult::Indirect {
                                convention: scoop_lir::IndirectResultConvention::CStoragePointer,
                                ..
                            }
                        ),
                        matches!(protocol, CallProtocol::NativeBorrowed { .. }),
                    ),
                };
                if !params_match || !result_matches || !protocol_matches {
                    return Err(CodegenError(format!(
                        "typed extern call @{} ABI does not match the declaration of `{}`",
                        self.function.symbol(),
                        declaration.source_name
                    )));
                }
            }
            scoop_lir::CallDestination::Runtime(_)
            | scoop_lir::CallDestination::Dispatch { .. } => {}
        }

        if let scoop_lir::CallDestination::Runtime(runtime) = destination {
            let (expected_params, expected_result, protocol_matches) = match runtime {
                scoop_lir::RuntimeFunction::Managed(function) => {
                    let shape = match function {
                        scoop_lir::ManagedRuntimeFunction::Safepoint
                        | scoop_lir::ManagedRuntimeFunction::GcCollect => {
                            (Vec::new(), LirType::Void)
                        }
                        scoop_lir::ManagedRuntimeFunction::Alloc => (
                            vec![
                                scoop_lir::METADATA_PTR,
                                LirType::MachineScalar(MachineScalarKind::ByteSize),
                            ],
                            scoop_lir::MANAGED_PTR,
                        ),
                        scoop_lir::ManagedRuntimeFunction::BoxZst
                        | scoop_lir::ManagedRuntimeFunction::BoxValue => {
                            return Err(CodegenError(
                                "boxing runtime calls require a descriptor-refined operation"
                                    .into(),
                            ));
                        }
                        scoop_lir::ManagedRuntimeFunction::MaterializeException => {
                            (vec![scoop_lir::MANAGED_PTR], scoop_lir::MANAGED_PTR)
                        }
                        scoop_lir::ManagedRuntimeFunction::StringConcat => (
                            vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                            scoop_lir::MANAGED_PTR,
                        ),
                        scoop_lir::ManagedRuntimeFunction::InitializationEnter => (
                            vec![scoop_lir::METADATA_PTR],
                            LirType::MachineScalar(MachineScalarKind::InitializationOutcome),
                        ),
                        scoop_lir::ManagedRuntimeFunction::InitializationSucceed => {
                            (vec![scoop_lir::METADATA_PTR], LirType::Void)
                        }
                        scoop_lir::ManagedRuntimeFunction::InitializationFail => (
                            vec![scoop_lir::METADATA_PTR, scoop_lir::MANAGED_PTR],
                            LirType::Void,
                        ),
                        scoop_lir::ManagedRuntimeFunction::InitializationFailure
                        | scoop_lir::ManagedRuntimeFunction::InitializationCycleMessage => {
                            (vec![scoop_lir::METADATA_PTR], scoop_lir::MANAGED_PTR)
                        }
                    };
                    (
                        shape.0,
                        shape.1,
                        matches!(
                            protocol,
                            CallProtocol::Managed { .. } | CallProtocol::ManagedInvoke { .. }
                        ),
                    )
                }
                scoop_lir::RuntimeFunction::NoGc(function) => {
                    let shape = match function {
                        scoop_lir::NoGcRuntimeFunction::UnboxZst
                        | scoop_lir::NoGcRuntimeFunction::UnboxValue
                        | scoop_lir::NoGcRuntimeFunction::PushRecursiveRegion
                        | scoop_lir::NoGcRuntimeFunction::PopRecursiveRegion => {
                            return Err(CodegenError(
                                "boxing runtime calls require a descriptor-refined operation"
                                    .into(),
                            ));
                        }
                        scoop_lir::NoGcRuntimeFunction::IsInstance => (
                            vec![scoop_lir::MANAGED_PTR, scoop_lir::METADATA_PTR],
                            LirType::I1,
                        ),
                        scoop_lir::NoGcRuntimeFunction::ITableLookup => (
                            vec![scoop_lir::METADATA_PTR, scoop_lir::METADATA_PTR],
                            scoop_lir::METADATA_PTR,
                        ),
                        scoop_lir::NoGcRuntimeFunction::Pin
                        | scoop_lir::NoGcRuntimeFunction::GetHandle => {
                            (vec![scoop_lir::MANAGED_PTR], LirType::I64)
                        }
                        scoop_lir::NoGcRuntimeFunction::Unpin
                        | scoop_lir::NoGcRuntimeFunction::ReleaseHandle => {
                            (vec![LirType::I64], scoop_lir::MANAGED_PTR)
                        }
                        scoop_lir::NoGcRuntimeFunction::GcStats => (Vec::new(), LirType::I64),
                        scoop_lir::NoGcRuntimeFunction::StringCompare => (
                            vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                            LirType::I64,
                        ),
                        scoop_lir::NoGcRuntimeFunction::Trap => {
                            (vec![scoop_lir::RAW_PTR], LirType::Void)
                        }
                        scoop_lir::NoGcRuntimeFunction::Throw => {
                            (vec![scoop_lir::MANAGED_PTR], LirType::Void)
                        }
                        scoop_lir::NoGcRuntimeFunction::Rethrow => (Vec::new(), LirType::Void),
                    };
                    (shape.0, shape.1, matches!(protocol, CallProtocol::NoGc))
                }
            };
            let result_matches = match result {
                TypedCallResult::Void => expected_result == LirType::Void,
                TypedCallResult::Direct { value, .. } => {
                    expected_result != LirType::Void && value.storage_type() == &expected_result
                }
                TypedCallResult::ElidedZst { .. } | TypedCallResult::Indirect { .. } => false,
            };
            let params_match = params.len() == expected_params.len()
                && params
                    .iter()
                    .zip(&expected_params)
                    .all(|(argument, expected)| {
                        matches!(argument, scoop_lir::AbiArgument::Direct(value) if value.storage_type() == expected)
                    });
            if !params_match || !result_matches || !protocol_matches {
                return Err(CodegenError(format!(
                    "typed runtime call @{} has a signature or protocol outside the closed runtime ABI for `{}`",
                    self.function.symbol(),
                    runtime.symbol()
                )));
            }
        }

        match result {
            TypedCallResult::Void => {}
            TypedCallResult::ElidedZst { out, value } => {
                if &self.function.temps[*out].ty != value.storage_type() {
                    return Err(CodegenError(format!(
                        "typed call @{}: elided ZST result temp does not match its signature",
                        self.function.symbol()
                    )));
                }
            }
            TypedCallResult::Direct { out, value } => {
                if &self.function.temps[*out].ty != value.storage_type() {
                    return Err(CodegenError(format!(
                        "typed call @{}: direct result temp does not match its signature",
                        self.function.symbol()
                    )));
                }
            }
            TypedCallResult::Indirect { storage, value, .. } => {
                if self.function.locals[*storage].ty() != value.storage_type() {
                    return Err(CodegenError(format!(
                        "typed call @{}: result storage does not match its signature",
                        self.function.symbol()
                    )));
                }
            }
        }

        Ok(())
    }
}
