use super::*;

mod protocol;

pub(super) use protocol::LoweredCallDestination;
use protocol::{NativeCallDestination, runtime_call_destination};

impl<'a> FunctionLowerer<'a> {
    pub(super) fn lower_call(
        &mut self,
        call: &mir::Call,
        result_ty: &mir::Type,
    ) -> Option<lir::Value> {
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
                    .collect::<Vec<_>>();
                match self.extern_function_refs[&id] {
                    LoweredExternFunctionRef::C(function) => {
                        let destination = NativeCallDestination::Safe(
                            lir::NativeSafeCallDestination::extern_function(function),
                        );
                        let mut bridge_args = Vec::with_capacity(args.len());
                        for (value, ty) in args.into_iter().zip(parameter_types) {
                            let ty = self.value_type(&ty);
                            let local = self.new_hidden_local(ty);
                            self.push(lir::Instruction::Store { local, value });
                            let address = self.new_temp(lir::RAW_PTR);
                            self.push(lir::Instruction::LocalAddress {
                                out: address,
                                local,
                            });
                            bridge_args.push(lir::Value::Temp(address));
                        }
                        let bridge_parameter_types = vec![lir::RAW_PTR; bridge_args.len()];
                        if returns_unit {
                            self.emit_native_call(
                                destination,
                                bridge_parameter_types,
                                lir::LirType::Void,
                                bridge_args,
                            )
                        } else {
                            let result_type = self.value_type(result_ty);
                            self.emit_native_storage_call(
                                destination,
                                bridge_parameter_types,
                                result_type,
                                lir::RefScan::None,
                                bridge_args,
                            )
                        }
                    }
                    LoweredExternFunctionRef::Scoop(function) => {
                        let destination = NativeCallDestination::Borrowed(
                            lir::NativeBorrowedCallDestination::extern_function(function),
                        );
                        let parameter_types = parameter_types
                            .iter()
                            .map(|ty| self.value_type(ty))
                            .collect();
                        let result_type = if returns_unit {
                            lir::LirType::Void
                        } else {
                            self.value_type(result_ty)
                        };
                        self.emit_native_call(destination, parameter_types, result_type, args)
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
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                let td = self.load_at_offset(
                    args[0],
                    self.context.object_type_descriptor_offset(),
                    lir::METADATA_PTR,
                );
                let target_td = self.td_ref(&mir::Type::Function(function_type));
                let table = self.emit_plain_call(
                    LoweredCallDestination::no_gc_runtime(lir::NoGcRuntimeFunction::ITableLookup),
                    vec![lir::METADATA_PTR, lir::METADATA_PTR],
                    lir::METADATA_PTR,
                    vec![lir::Value::Temp(td), target_td],
                );
                let destination =
                    self.managed_dispatch_destination(table, lir::DispatchKind::FunctionBridge, 0);
                self.finish_indirect(
                    destination,
                    args,
                    parameter_types,
                    !signature.is_suspend && signature.return_type == mir::Type::Unit,
                    result_ty,
                )
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
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                self.finish_closure(
                    args[0],
                    args,
                    parameter_types,
                    !signature.is_suspend && signature.return_type == mir::Type::Unit,
                    result_ty,
                )
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
                    mir::Callee::Extern(_) => unreachable!("handled above"),
                };
                let callee = &self.module.functions[id];
                let param_types: Vec<mir::Type> =
                    callee.params.iter().map(|param| param.ty.clone()).collect();
                let returns_unit = callee.return_ty == mir::Type::Unit;
                assert_eq!(call.args.len(), param_types.len(), "user call arity");
                // Arguments are evaluated left to right, before the call.
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                match call.target.kind {
                    mir::CallKind::Direct => {
                        let destination =
                            LoweredCallDestination::local(self.local_function_map[&id]);
                        self.finish_call(destination, args, param_types, returns_unit, result_ty)
                    }
                    // vtable dispatch (impl spec 2.9): the receiver's
                    // object header holds the TypeDescriptor, whose
                    // vtable pointer is `ScoopTypeDescriptor` field 5.
                    mir::CallKind::Virtual { slot } => {
                        let td = self.load_at_offset(
                            args[0],
                            self.context.object_type_descriptor_offset(),
                            lir::METADATA_PTR,
                        );
                        let vtable = self.load_at_offset(
                            lir::Value::Temp(td),
                            self.context.type_descriptor_vtable_offset(),
                            lir::METADATA_PTR,
                        );
                        let destination = self.dispatch_destination(
                            lir::Value::Temp(vtable),
                            lir::DispatchKind::Virtual,
                            slot,
                            callee.gc_effect,
                        );
                        self.finish_indirect(
                            destination,
                            args,
                            param_types,
                            returns_unit,
                            result_ty,
                        )
                    }
                    // itable dispatch: `scoop_rt_itable_lookup(td,
                    // iface_td)` finds the interface's table by its
                    // TypeDescriptor key.
                    mir::CallKind::Interface { interface, slot } => {
                        let td = self.load_at_offset(
                            args[0],
                            self.context.object_type_descriptor_offset(),
                            lir::METADATA_PTR,
                        );
                        let iface_td = self.td_ref(&mir::Type::Interface(interface));
                        let table = self.emit_plain_call(
                            LoweredCallDestination::no_gc_runtime(
                                lir::NoGcRuntimeFunction::ITableLookup,
                            ),
                            vec![lir::METADATA_PTR, lir::METADATA_PTR],
                            lir::METADATA_PTR,
                            vec![lir::Value::Temp(td), iface_td],
                        );
                        let destination = self.dispatch_destination(
                            table,
                            lir::DispatchKind::Interface,
                            slot,
                            callee.gc_effect,
                        );
                        self.finish_indirect(
                            destination,
                            args,
                            param_types,
                            returns_unit,
                            result_ty,
                        )
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
            mir::Callee::Runtime(mir::RuntimeFn::Trap) => {
                // The trap call (from `!!`) only appears as a
                // statement: the current block branches to the
                // function's shared trap block and is sealed, so
                // anything after it is unreachable. The message string
                // constant becomes a `CString` global.
                let message = match &call.args[0].kind {
                    mir::ExprKind::StringConst(id) => self.module.strings[*id].value.clone(),
                    _ => unreachable!("the trap message is a string constant"),
                };
                let trap = self.trap_block(&message);
                self.seal(lir::Terminator::Br(trap));
                self.current_sealed = true;
                return None;
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
                    // Box / IsInstance / dispatch lowerings, never as plain
                    // MIR calls.
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                    // Handled by the arm above.
                    mir::RuntimeFn::Trap => unreachable!("trap calls never reach here"),
                };
                assert_eq!(call.args.len(), expected_arg_count, "runtime call arity");
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                let (parameter_types, result_type) = match function {
                    mir::RuntimeFn::StringConcat => {
                        (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::StringCompare => {
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
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                    mir::RuntimeFn::Trap => unreachable!("trap calls never reach here"),
                };
                let function = lower_runtime_function(function);
                self.emit_plain_call(
                    runtime_call_destination(function),
                    parameter_types,
                    result_type,
                    args,
                )
            }
        };
        Some(value)
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
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        let parameter_types = parameter_types
            .iter()
            .map(|ty| self.value_type(ty))
            .collect();
        let result_type = if returns_unit {
            lir::LirType::Void
        } else {
            self.value_type(result_ty)
        };
        self.emit_non_native_call(destination, parameter_types, result_type, args)
    }

    /// An indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9). Inside a try body it is invoked to
    /// the innermost landing pad, like `finish_call`.
    pub(super) fn finish_indirect(
        &mut self,
        destination: LoweredCallDestination,
        args: Vec<lir::Value>,
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        self.finish_call(destination, args, parameter_types, returns_unit, result_ty)
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
    ) -> lir::Value {
        let destination = self.managed_dispatch_destination(
            closure,
            lir::DispatchKind::Closure,
            self.context.closure_invoke_dispatch_slot(),
        );
        self.finish_indirect(destination, args, parameter_types, returns_unit, result_ty)
    }
}
