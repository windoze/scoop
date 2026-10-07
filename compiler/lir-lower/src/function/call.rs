use super::*;

mod dispatch;
mod emission;
mod protocol;

pub(super) use protocol::LoweredCallDestination;
use protocol::{NativeCallDestination, runtime_call_destination};

impl<'a> FunctionLowerer<'a> {
    pub(super) fn lower_call(
        &mut self,
        call: &mir::Call,
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let value = match call.target.callee {
            mir::Callee::Extern(id) => {
                assert!(matches!(call.target.kind, mir::CallKind::Direct));
                let extern_ = &self.module.extern_functions[id];
                let parameter_types = extern_.params.clone();
                let returns_unit = extern_.return_type == mir::Type::Unit;
                assert_eq!(call.args.len(), parameter_types.len(), "extern call arity");
                let args = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                match self.extern_function_refs[&id] {
                    LoweredExternFunctionRef::C(function) => {
                        let lir::ExternFunctionKind::C { call_mode, .. } =
                            self.extern_functions[function.declaration()].kind
                        else {
                            unreachable!("C extern reference names a C declaration")
                        };
                        let destination = NativeCallDestination::C(
                            lir::CCallDestination::extern_function(function),
                            call_mode,
                        );
                        let mut bridge_args = Vec::with_capacity(args.len());
                        for (value, ty) in args.into_iter().zip(parameter_types) {
                            let value = self.project_c_value(&ty, value);
                            let ty = self.c_storage_type(&ty);
                            let local = self.new_hidden_local(ty)?;
                            self.push(lir::Instruction::Store { local, value });
                            bridge_args.push(lir::Value::CArgumentStorage(
                                lir::CArgumentStorage::address_of(local),
                            ));
                        }
                        let bridge_parameter_types = vec![lir::RAW_PTR; bridge_args.len()];
                        if returns_unit {
                            self.emit_native_call(
                                destination,
                                bridge_parameter_types,
                                lir::LirType::Void,
                                bridge_args,
                            )?
                        } else {
                            let result_type = self.c_storage_type(result_ty);
                            let result = self.emit_native_storage_call(
                                destination,
                                bridge_parameter_types,
                                result_type,
                                lir::RefScan::None,
                                bridge_args,
                            )?;
                            self.restore_c_value(result_ty, result)
                        }
                    }
                    LoweredExternFunctionRef::Scoop(function) => {
                        let destination = NativeCallDestination::Borrowed(
                            lir::NativeBorrowedCallDestination::extern_function(function),
                        );
                        let signature = match &self.extern_functions[function.declaration()].kind {
                            lir::ExternFunctionKind::Scoop { signature, .. } => signature.clone(),
                            lir::ExternFunctionKind::C { .. } => {
                                unreachable!("Scoop extern reference names a Scoop declaration")
                            }
                        };
                        self.emit_native_call_with_signature(destination, &signature, args)?
                    }
                }
            }
            mir::Callee::FunctionBridge(function_type) => {
                let signature = self.module.function_types[function_type].clone();
                let mut parameter_types = Vec::with_capacity(call.args.len());
                parameter_types.push(mir::Type::Any);
                parameter_types.extend(signature.parameter_types);
                for arg in call.args.iter().skip(parameter_types.len()) {
                    parameter_types.push(arg.ty.clone());
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend function bridges add a hidden continuation argument"
                );
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                let td = self.load_at_offset(
                    args[0],
                    self.context.object_type_descriptor_offset(),
                    lir::METADATA_PTR,
                );
                let table = self.load_at_offset(
                    lir::Value::Temp(td),
                    self.context.type_descriptor_vtable_offset(),
                    lir::METADATA_PTR,
                );
                let destination = self.managed_dispatch_destination(
                    lir::Value::Temp(table),
                    lir::DispatchKind::FunctionBridge,
                    0,
                );
                let returns_unit =
                    !signature.is_suspend && signature.return_type == mir::Type::Unit;
                let call_signature = abi::classify_mir_signature(
                    self.context,
                    self.module,
                    parameter_types.iter(),
                    if returns_unit {
                        &mir::Type::Unit
                    } else {
                        result_ty
                    },
                    self.structs,
                    self.enums,
                )?;
                self.finish_indirect(destination, args, &call_signature)?
            }
            mir::Callee::External(source) => {
                self.lower_external_call(call, self.external_callable_map[&source])?
            }
            mir::Callee::Closure(function_type) => {
                let signature = self.module.function_types[function_type].clone();
                let mut parameter_types = Vec::with_capacity(call.args.len());
                parameter_types.push(mir::Type::Function(function_type));
                parameter_types.extend(signature.parameter_types);
                for arg in call.args.iter().skip(parameter_types.len()) {
                    parameter_types.push(arg.ty.clone());
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend closure calls add a hidden continuation argument"
                );
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                self.finish_closure(
                    args[0],
                    args,
                    parameter_types,
                    !signature.is_suspend && signature.return_type == mir::Type::Unit,
                    result_ty,
                )?
            }
            mir::Callee::User(_) | mir::Callee::Monomorphized(_) => {
                let id = match call.target.callee {
                    mir::Callee::User(id) => id,
                    mir::Callee::Monomorphized(instance) => {
                        self.module.meta.instances[instance].function
                    }
                    mir::Callee::CoroutineSuspend { .. } | mir::Callee::Runtime(_) => {
                        unreachable!("matched a local callee above")
                    }
                    mir::Callee::Closure(_) | mir::Callee::FunctionBridge(_) => {
                        unreachable!("handled above")
                    }
                    mir::Callee::External(_) => unreachable!("handled above"),
                    mir::Callee::Extern(_) => unreachable!("handled above"),
                };
                let callee = &self.module.functions[id];
                let signature = self.function_signatures[&id].clone();
                assert_eq!(
                    call.args.len(),
                    signature.logical_argument_count(),
                    "user call arity"
                );
                // Arguments are evaluated left to right, before the call.
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                match &call.target.kind {
                    mir::CallKind::Direct => {
                        let destination =
                            LoweredCallDestination::local(self.local_function_map[&id]);
                        self.finish_call(destination, args, &signature)?
                    }
                    kind @ (mir::CallKind::Virtual { .. } | mir::CallKind::Interface { .. }) => {
                        let destination =
                            self.load_dispatch_destination(kind, args[0], callee.gc_effect)?;
                        self.finish_indirect(destination, args, &signature)?
                    }
                    mir::CallKind::Closure { .. } => {
                        unreachable!("closure calls have no statically selected user callee")
                    }
                    mir::CallKind::FunctionBridge { .. } => {
                        unreachable!("function bridge calls have no static user callee")
                    }
                }
            }
            mir::Callee::CoroutineSuspend { .. } => {
                unreachable!("coroutine state-machine lowering removes suspend markers")
            }
            mir::Callee::Runtime(function) => {
                let expected_arg_count = match function {
                    mir::RuntimeFn::StringConcat | mir::RuntimeFn::StringCompare => 2,
                    // The GC intrinsics (M9, runtime spec 3.4): the
                    // pin / handle operations speak raw machine words
                    // — the object reference in, the word out (or the
                    // reverse); the hooks take nothing.
                    mir::RuntimeFn::Pin
                    | mir::RuntimeFn::GetHandle
                    | mir::RuntimeFn::Unpin
                    | mir::RuntimeFn::ReleaseHandle
                    | mir::RuntimeFn::MaterializeException => 1,
                    mir::RuntimeFn::InitializationEnter
                    | mir::RuntimeFn::InitializationSucceed
                    | mir::RuntimeFn::InitializationFailure
                    | mir::RuntimeFn::InitializationCycleMessage => 1,
                    mir::RuntimeFn::InitializationFail => 2,
                    mir::RuntimeFn::GcCollect | mir::RuntimeFn::GcStats => 0,
                    // The M6 runtime functions are emitted by dedicated
                    // IsInstance / dispatch lowerings, never as plain
                    // MIR calls.
                    mir::RuntimeFn::IsInstance | mir::RuntimeFn::ITableLookup => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                };
                assert_eq!(call.args.len(), expected_arg_count, "runtime call arity");
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<StorageResult<Vec<_>>>()?;
                let (parameter_types, result_type) = match function {
                    mir::RuntimeFn::StringConcat => {
                        (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::StringCompare => {
                        assert_eq!(
                            *result_ty,
                            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
                            "the closed string-compare runtime ABI returns signed 64-bit",
                        );
                        (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::LirType::I64)
                    }
                    // The pin / handle intrinsics exchange a word with
                    // the runtime: `pin` / `getGcHandle` yield the raw
                    // word (i64), `unpin` / `releaseGcHandle` yield the
                    // reference (ptr), `gcStats` yields the count.
                    mir::RuntimeFn::Pin | mir::RuntimeFn::GetHandle | mir::RuntimeFn::GcStats => {
                        let params = if function == mir::RuntimeFn::GcStats {
                            Vec::new()
                        } else {
                            vec![lir::MANAGED_PTR]
                        };
                        (params, lir::LirType::I64)
                    }
                    mir::RuntimeFn::Unpin | mir::RuntimeFn::ReleaseHandle => {
                        (vec![lir::LirType::I64], lir::MANAGED_PTR)
                    }
                    // `BeginCatch` publishes the registered stable external
                    // exception object as a managed reference. The runtime
                    // materializer consumes that same typed reference even
                    // though its base is outside the moving heap.
                    mir::RuntimeFn::MaterializeException => {
                        (vec![lir::MANAGED_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::InitializationEnter => (
                        vec![lir::METADATA_PTR],
                        lir::LirType::MachineScalar(lir::MachineScalarKind::InitializationOutcome),
                    ),
                    mir::RuntimeFn::InitializationSucceed => {
                        (vec![lir::METADATA_PTR], lir::LirType::Void)
                    }
                    mir::RuntimeFn::InitializationFail => (
                        vec![lir::METADATA_PTR, lir::MANAGED_PTR],
                        lir::LirType::Void,
                    ),
                    mir::RuntimeFn::InitializationFailure => {
                        (vec![lir::METADATA_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::InitializationCycleMessage => {
                        (vec![lir::METADATA_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::GcCollect => (Vec::new(), lir::LirType::Void),
                    mir::RuntimeFn::IsInstance | mir::RuntimeFn::ITableLookup => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                };
                let function = lower_runtime_function(function);
                self.emit_plain_call(
                    runtime_call_destination(function),
                    parameter_types,
                    result_type,
                    args,
                )?
            }
        };
        Ok(value)
    }

    fn lower_external_call(
        &mut self,
        call: &mir::Call,
        id: lir::ExternalCallableId,
    ) -> StorageResult<lir::Value> {
        let callable = &self.external_callables[id];
        assert_eq!(
            call.args.len(),
            callable.signature().logical_argument_count(),
            "external call arity"
        );
        let args = call
            .args
            .iter()
            .map(|argument| self.lower_expr(argument))
            .collect::<StorageResult<Vec<_>>>()?;
        let effect = callable.gc_effect();
        let signature = callable.signature().clone();
        let destination = match &call.target.kind {
            mir::CallKind::Direct => LoweredCallDestination::external(id, effect),
            kind @ (mir::CallKind::Virtual { .. } | mir::CallKind::Interface { .. }) => {
                let effect = match effect {
                    lir::GcEffect::Managed => mir::GcEffect::Managed,
                    lir::GcEffect::NoGc => mir::GcEffect::NoGc,
                };
                self.load_dispatch_destination(kind, args[0], effect)?
            }
            mir::CallKind::Closure { .. } | mir::CallKind::FunctionBridge { .. } => {
                unreachable!("external declarations do not use closure dispatch")
            }
        };
        self.emit_non_native_call_with_signature(destination, &signature, args)
    }

    /// Load a value of `ty` at a fixed byte offset from a raw pointer.
    pub(super) fn load_at_offset(
        &mut self,
        object: lir::Value,
        offset: u64,
        ty: lir::LirType,
    ) -> lir::TempId {
        let out = self.new_temp(ty.clone());
        match ty {
            lir::LirType::MachineScalar(kind) => {
                assert!(
                    kind.is_atomic_state(),
                    "only compiler-owned coroutine state has heap storage"
                );
                self.push(lir::Instruction::MachineHeapLoad {
                    out,
                    kind,
                    object,
                    offset,
                });
            }
            _ => self.push(lir::Instruction::HeapLoad {
                out,
                object,
                offset,
            }),
        }
        out
    }

    /// Store a value of `ty` at a fixed byte offset. Compiler-owned state uses
    /// a closed instruction family so it cannot pass through a source `i64`
    /// heap operation before final LLVM lowering.
    pub(super) fn store_at_offset(
        &mut self,
        object: lir::Value,
        offset: u64,
        value: lir::Value,
        ty: lir::LirType,
    ) {
        match ty {
            lir::LirType::MachineScalar(kind) => {
                assert!(
                    kind.is_atomic_state(),
                    "only compiler-owned coroutine state has heap storage"
                );
                self.push(lir::Instruction::MachineHeapStore {
                    kind,
                    object,
                    offset,
                    value,
                });
            }
            _ => self.push(lir::Instruction::HeapStore {
                object,
                offset,
                value,
            }),
        }
    }

    /// A direct call: Unit-returning callees are void at the LLVM
    /// level; their Unit value is a fresh empty aggregate. Inside a
    /// try body the call may throw, so it is invoked to the innermost
    /// landing pad (M8): the `Invoke` ends the block (the terminator
    /// convention in the module docs) and the result is available in
    /// the normal successor.
    pub(super) fn finish_call(
        &mut self,
        destination: LoweredCallDestination,
        args: Vec<lir::Value>,
        signature: &lir::ScoopAbiSignature,
    ) -> StorageResult<lir::Value> {
        self.emit_non_native_call_with_signature(destination, signature, args)
    }

    /// An indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9). Inside a try body it is invoked to
    /// the innermost landing pad, like `finish_call`.
    pub(super) fn finish_indirect(
        &mut self,
        destination: LoweredCallDestination,
        args: Vec<lir::Value>,
        signature: &lir::ScoopAbiSignature,
    ) -> StorageResult<lir::Value> {
        self.finish_call(destination, args, signature)
    }

    /// A managed closure call through the code pointer already loaded from
    /// the closure object. Its unwind behavior is identical to direct and
    /// table-indirect managed calls.
    pub(super) fn finish_closure(
        &mut self,
        closure: lir::Value,
        args: Vec<lir::Value>,
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let destination = self.managed_dispatch_destination(
            closure,
            lir::DispatchKind::Closure,
            self.context.closure_invoke_dispatch_slot(),
        );
        let signature = abi::classify_mir_signature(
            self.context,
            self.module,
            parameter_types.iter(),
            if returns_unit {
                &mir::Type::Unit
            } else {
                result_ty
            },
            self.structs,
            self.enums,
        )?;
        self.finish_indirect(destination, args, &signature)
    }
}
