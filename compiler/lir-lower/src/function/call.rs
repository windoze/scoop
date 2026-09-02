use super::*;

impl<'a> FunctionLowerer<'a> {
    pub(super) fn typed_call(
        &mut self,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> (PendingTypedCall, Option<lir::Value>) {
        assert_eq!(parameter_types.len(), args.len(), "typed call arity");
        let calling_convention = lir::CallingConvention::Cdecl;
        if result_type == lir::LirType::Void {
            let signature = self
                .call_targets
                .void_signatures
                .alloc(lir::VoidCallSignature {
                    params: parameter_types,
                    calling_convention,
                });
            return (PendingTypedCall::Void { signature, args }, None);
        }

        let result_scan = safepoints::root_scan(&result_type, self.structs, self.enums, 0);
        if uses_indirect_result(self.enums, &result_type) {
            let signature = self.call_targets.indirect_result_signatures.alloc(
                lir::IndirectResultCallSignature {
                    params: parameter_types,
                    result: lir::ResultStorage {
                        ty: result_type.clone(),
                        scan: result_scan,
                    },
                    calling_convention,
                },
            );
            let storage = self.new_hidden_local(result_type);
            return (
                PendingTypedCall::IndirectResult {
                    signature,
                    storage,
                    args,
                },
                Some(lir::Value::Local(storage)),
            );
        }

        let signature = self
            .call_targets
            .direct_signatures
            .alloc(lir::DirectCallSignature {
                params: parameter_types,
                result: result_type.clone(),
                result_scan,
                calling_convention,
            });
        let out = self.new_temp(result_type);
        (
            PendingTypedCall::Direct {
                signature,
                out,
                args,
            },
            Some(lir::Value::Temp(out)),
        )
    }

    pub(super) fn call_site(
        &mut self,
        destination: LoweredCallDestination,
        call: PendingTypedCall,
    ) -> lir::CallSite {
        match destination {
            LoweredCallDestination::Managed(destination) => {
                lir::CallSite::Managed(lir::ManagedCallSite {
                    call: bind_typed_call(
                        &mut self.call_targets.managed_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    live: lir::StatepointLiveSet::default(),
                })
            }
            LoweredCallDestination::NoGc(destination) => lir::CallSite::NoGc(lir::NoGcCallSite {
                call: bind_typed_call(&mut self.call_targets.no_gc_targets, destination, call),
            }),
        }
    }

    pub(super) fn native_call_site(
        &mut self,
        destination: NativeCallDestination,
        call: PendingTypedCall,
    ) -> lir::CallSite {
        match destination {
            NativeCallDestination::Safe(destination) => {
                assert_eq!(
                    call.result_scan(&self.call_targets),
                    &lir::RefScan::None,
                    "native-safe C ABI results must be GC-free"
                );
                lir::CallSite::NativeSafe(lir::NativeSafeCallSite {
                    call: bind_typed_call(
                        &mut self.call_targets.native_safe_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::NativeSafeRootSet::default(),
                })
            }
            NativeCallDestination::Borrowed(destination) => {
                let result_scan = call.result_scan(&self.call_targets).clone();
                let result = match lir::NonEmptyRefScan::new(result_scan) {
                    None => lir::NativeBorrowedResultRoot::GcFree,
                    Some(_) => {
                        let storage = match &call {
                            PendingTypedCall::Void { .. } => {
                                unreachable!("void native call cannot have a result root")
                            }
                            PendingTypedCall::Direct { signature, .. } => {
                                let ty = self.call_targets.direct_signatures[*signature]
                                    .result
                                    .clone();
                                self.new_hidden_local(ty)
                            }
                            PendingTypedCall::IndirectResult { storage, .. } => *storage,
                        };
                        lir::NativeBorrowedResultRoot::Rooted { storage }
                    }
                };
                let call = bind_typed_call(
                    &mut self.call_targets.native_borrowed_targets,
                    destination,
                    call,
                );
                let call = self.call_targets.bind_native_borrowed_call(call, result);
                lir::CallSite::NativeBorrowed(lir::NativeBorrowedCallSite {
                    call,
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::NativeBorrowedRootSet::default(),
                })
            }
        }
    }

    pub(super) fn invoke_site(
        &mut self,
        destination: LoweredCallDestination,
        call: PendingTypedCall,
        normal: lir::BlockId,
        unwind: lir::BlockId,
    ) -> lir::InvokeSite {
        match destination {
            LoweredCallDestination::Managed(destination) => {
                lir::InvokeSite::Managed(lir::ManagedInvokeSite {
                    call: bind_typed_call(
                        &mut self.call_targets.managed_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::ExceptionalRootSet::default(),
                    normal,
                    unwind,
                })
            }
            LoweredCallDestination::NoGc(destination) => {
                lir::InvokeSite::NoGc(lir::NoGcInvokeSite {
                    call: bind_typed_call(&mut self.call_targets.no_gc_targets, destination, call),
                    normal,
                    unwind,
                })
            }
        }
    }

    pub(super) fn managed_dispatch_destination(
        &mut self,
        table: lir::Value,
        kind: lir::DispatchKind,
        index: u32,
    ) -> LoweredCallDestination {
        let slot = self
            .call_targets
            .dispatch_slots
            .alloc_managed(lir::DispatchSlot { kind, index });
        LoweredCallDestination::Managed(lir::ManagedCallDestination::dispatch(table, slot))
    }

    pub(super) fn dispatch_destination(
        &mut self,
        table: lir::Value,
        kind: lir::DispatchKind,
        index: u32,
        effect: mir::GcEffect,
    ) -> LoweredCallDestination {
        match effect {
            mir::GcEffect::Managed => {
                let slot = self
                    .call_targets
                    .dispatch_slots
                    .alloc_managed(lir::DispatchSlot { kind, index });
                LoweredCallDestination::Managed(lir::ManagedCallDestination::dispatch(table, slot))
            }
            mir::GcEffect::NoGc => {
                let slot = self
                    .call_targets
                    .dispatch_slots
                    .alloc_no_gc(lir::DispatchSlot { kind, index });
                LoweredCallDestination::NoGc(lir::NoGcCallDestination::dispatch(table, slot))
            }
        }
    }

    pub(super) fn emit_non_native_call(
        &mut self,
        destination: LoweredCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        if let Some(unwind) = self.current_unwind {
            let normal = self.new_block("invoke.normal");
            let site = self.invoke_site(destination, call, normal, unwind);
            self.push(lir::Instruction::Invoke { site });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
        } else {
            let site = self.call_site(destination, call);
            self.push(lir::Instruction::Call { site });
        }
        value.unwrap_or_else(|| self.unit_value())
    }

    pub(super) fn emit_plain_call(
        &mut self,
        destination: LoweredCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        let site = self.call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        value.unwrap_or_else(|| self.unit_value())
    }

    pub(super) fn emit_native_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        let site = self.native_call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        value.unwrap_or_else(|| self.unit_value())
    }

    pub(super) fn emit_native_storage_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        result_scan: lir::RefScan,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        assert_ne!(result_type, lir::LirType::Void);
        assert_eq!(parameter_types.len(), args.len(), "typed call arity");
        let signature =
            self.call_targets
                .indirect_result_signatures
                .alloc(lir::IndirectResultCallSignature {
                    params: parameter_types,
                    result: lir::ResultStorage {
                        ty: result_type.clone(),
                        scan: result_scan,
                    },
                    calling_convention: lir::CallingConvention::Cdecl,
                });
        let storage = self.new_hidden_local(result_type);
        let call = PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        };
        let site = self.native_call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        lir::Value::Local(storage)
    }

    pub(super) fn lower_call(&mut self, call: &mir::Call, result_ty: &mir::Type) -> lir::Value {
        match call.target.callee {
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
                let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
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
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                        let vtable =
                            self.load_at_offset(lir::Value::Temp(td), 5 * 8, lir::METADATA_PTR);
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
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
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
                // Dead value: the block is sealed, nothing consumes it.
                lir::Value::IntConst(0)
            }
            mir::Callee::Runtime(function) => {
                let expected_arg_count = match function {
                    mir::RuntimeFn::StringConcat => 2,
                    // The GC intrinsics (M9, runtime spec 3.4): the
                    // pin / handle operations speak raw machine words
                    // — the object reference in, the word out (or the
                    // reverse); the hooks take nothing.
                    mir::RuntimeFn::Pin
                    | mir::RuntimeFn::GetHandle
                    | mir::RuntimeFn::Unpin
                    | mir::RuntimeFn::ReleaseHandle
                    | mir::RuntimeFn::MaterializeException => 1,
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
        }
    }

    /// Load a value of `ty` at a fixed byte offset from a raw pointer.
    pub(super) fn load_at_offset(
        &mut self,
        object: lir::Value,
        offset: u64,
        ty: lir::LirType,
    ) -> lir::TempId {
        let out = self.new_temp(ty);
        self.push(lir::Instruction::HeapLoad {
            out,
            object,
            offset,
        });
        out
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
        let destination = self.managed_dispatch_destination(closure, lir::DispatchKind::Closure, 2);
        self.finish_indirect(destination, args, parameter_types, returns_unit, result_ty)
    }
}
