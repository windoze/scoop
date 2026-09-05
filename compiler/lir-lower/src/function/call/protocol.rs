//! Typed call signatures and protocol-specific call-site binding.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::function) enum LoweredCallDestination {
    Managed(lir::ManagedCallDestination),
    NoGc(lir::NoGcCallDestination),
}

impl LoweredCallDestination {
    pub(in crate::function) fn managed_runtime(function: lir::ManagedRuntimeFunction) -> Self {
        Self::Managed(lir::ManagedCallDestination::runtime(function))
    }

    pub(in crate::function) fn no_gc_runtime(function: lir::NoGcRuntimeFunction) -> Self {
        Self::NoGc(lir::NoGcCallDestination::runtime(function))
    }

    pub(in crate::function) fn local(function: lir::LocalFunctionRef) -> Self {
        match function {
            lir::LocalFunctionRef::Managed(function) => {
                Self::Managed(lir::ManagedCallDestination::local(function))
            }
            lir::LocalFunctionRef::NoGc(function) => {
                Self::NoGc(lir::NoGcCallDestination::local(function))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::function) enum NativeCallDestination {
    Safe(lir::NativeSafeCallDestination),
    Borrowed(lir::NativeBorrowedCallDestination),
}

/// A fully typed call shape before its protocol-specific destination is bound.
/// This is lowering-local scratch state and can never enter LIR output.
pub(in crate::function) enum PendingTypedCall {
    Void {
        signature: lir::VoidCallSignatureId,
        args: Vec<lir::Value>,
    },
    Direct {
        signature: lir::DirectCallSignatureId,
        out: lir::TempId,
        args: Vec<lir::Value>,
    },
    IndirectResult {
        signature: lir::IndirectResultCallSignatureId,
        storage: lir::LocalId,
        args: Vec<lir::Value>,
    },
}

impl PendingTypedCall {
    fn result_scan<'a>(&self, targets: &'a lir::CallTargets) -> &'a lir::RefScan {
        match self {
            Self::Void { .. } => &lir::RefScan::None,
            Self::Direct { signature, .. } => &targets.direct_signatures[*signature].result_scan,
            Self::IndirectResult { signature, .. } => {
                &targets.indirect_result_signatures[*signature].result.scan
            }
        }
    }
}

fn bind_typed_call<Destination: Copy>(
    targets: &mut lir::ProtocolCallTargets<Destination>,
    destination: Destination,
    call: PendingTypedCall,
) -> lir::TypedCall<Destination> {
    match call {
        PendingTypedCall::Void { signature, args } => {
            let target = targets.void.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Void { target, args }
        }
        PendingTypedCall::Direct {
            signature,
            out,
            args,
        } => {
            let target = targets.direct.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Direct { target, out, args }
        }
        PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        } => {
            let target = targets.indirect_result.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::IndirectResult {
                target,
                storage,
                args,
            }
        }
    }
}

pub(in crate::function) fn runtime_call_destination(
    function: lir::RuntimeFunction,
) -> LoweredCallDestination {
    match function {
        lir::RuntimeFunction::Managed(function) => {
            LoweredCallDestination::managed_runtime(function)
        }
        lir::RuntimeFunction::NoGc(function) => LoweredCallDestination::no_gc_runtime(function),
    }
}

impl<'a> FunctionLowerer<'a> {
    pub(in crate::function) fn typed_call(
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

        let result_scan =
            safepoints::root_scan(self.context, &result_type, self.structs, self.enums, 0);
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

    pub(in crate::function) fn call_site(
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

    pub(in crate::function) fn native_call_site(
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

    pub(in crate::function) fn invoke_site(
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

    pub(in crate::function) fn managed_dispatch_destination(
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

    pub(in crate::function) fn dispatch_destination(
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

    pub(in crate::function) fn emit_non_native_call(
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

    pub(in crate::function) fn emit_plain_call(
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

    pub(in crate::function) fn emit_native_call(
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

    pub(in crate::function) fn emit_native_storage_call(
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
}
