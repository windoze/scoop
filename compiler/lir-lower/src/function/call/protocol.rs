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

    pub(in crate::function) fn external(
        function: lir::ExternalCallableId,
        effect: lir::GcEffect,
    ) -> Self {
        match effect {
            lir::GcEffect::Managed => {
                Self::Managed(lir::ManagedCallDestination::external(function))
            }
            lir::GcEffect::NoGc => Self::NoGc(lir::NoGcCallDestination::external(function)),
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
        args: Vec<lir::AbiCallArgument>,
    },
    ElidedZst {
        signature: lir::ElidedZstCallSignatureId,
        out: lir::TempId,
        args: Vec<lir::AbiCallArgument>,
    },
    Direct {
        signature: lir::DirectCallSignatureId,
        out: lir::TempId,
        args: Vec<lir::AbiCallArgument>,
    },
    IndirectResult {
        signature: lir::IndirectResultCallSignatureId,
        storage: lir::LocalId,
        args: Vec<lir::AbiCallArgument>,
    },
}

impl PendingTypedCall {
    fn result_scan<'a>(&self, targets: &'a lir::CallTargets) -> &'a lir::RefScan {
        match self {
            Self::Void { .. } | Self::ElidedZst { .. } => &lir::RefScan::None,
            Self::Direct { signature, .. } => targets.direct_signatures[*signature].result().scan(),
            Self::IndirectResult { signature, .. } => targets.indirect_result_signatures
                [*signature]
                .result()
                .scan(),
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
        PendingTypedCall::ElidedZst {
            signature,
            out,
            args,
        } => {
            let target = targets.elided_zst.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::ElidedZst { target, out, args }
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
    fn classified_call_signature(
        &self,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
    ) -> StorageResult<lir::ScoopAbiSignature> {
        abi::classify_signature(
            self.context,
            parameter_types,
            (result_type != lir::LirType::Void).then_some(result_type),
            self.structs,
            self.enums,
        )
    }

    pub(in crate::function) fn typed_call(
        &mut self,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> StorageResult<(PendingTypedCall, Option<lir::Value>)> {
        let signature = self.classified_call_signature(parameter_types, result_type)?;
        self.typed_call_with_signature(&signature, args)
    }

    pub(in crate::function) fn typed_call_with_signature(
        &mut self,
        signature: &lir::ScoopAbiSignature,
        args: Vec<lir::Value>,
    ) -> StorageResult<(PendingTypedCall, Option<lir::Value>)> {
        assert_eq!(
            signature.logical_argument_count(),
            args.len(),
            "typed call arity"
        );
        let mut lowered_args = Vec::with_capacity(args.len());
        for (argument, value) in signature.arguments().iter().zip(args) {
            lowered_args.push(match argument {
                lir::AbiArgument::ElidedZst(_) => lir::AbiCallArgument::ElidedZst(value),
                lir::AbiArgument::Direct(_) => lir::AbiCallArgument::Direct(value),
                lir::AbiArgument::Indirect(expected) => {
                    let storage = self.new_hidden_local(expected.storage_type().clone())?;
                    self.push(lir::Instruction::Store {
                        local: storage,
                        value,
                    });
                    lir::AbiCallArgument::Indirect(
                        lir::AbiArgumentStorage::new(storage, self.locals[storage].ty(), expected)
                            .expect("fresh indirect argument storage has its exact ABI type"),
                    )
                }
            });
        }

        let arguments = signature.arguments().to_vec();
        let calling_convention = signature.calling_convention();
        Ok(match signature.result() {
            lir::AbiReturn::UnitVoid => {
                let signature = self
                    .call_targets
                    .void_signatures
                    .alloc(lir::VoidCallSignature::new(arguments, calling_convention));
                (
                    PendingTypedCall::Void {
                        signature,
                        args: lowered_args,
                    },
                    None,
                )
            }
            lir::AbiReturn::ElidedZst(result) => {
                let out = self.new_temp(result.storage_type().clone());
                let signature = self.call_targets.elided_zst_signatures.alloc(
                    lir::ElidedZstCallSignature::new(arguments, result.clone(), calling_convention),
                );
                (
                    PendingTypedCall::ElidedZst {
                        signature,
                        out,
                        args: lowered_args,
                    },
                    Some(lir::Value::Temp(out)),
                )
            }
            lir::AbiReturn::Direct(result) => {
                let out = self.new_temp(result.storage_type().clone());
                let signature =
                    self.call_targets
                        .direct_signatures
                        .alloc(lir::DirectCallSignature::new(
                            arguments,
                            result.clone(),
                            calling_convention,
                        ));
                (
                    PendingTypedCall::Direct {
                        signature,
                        out,
                        args: lowered_args,
                    },
                    Some(lir::Value::Temp(out)),
                )
            }
            lir::AbiReturn::Indirect(result) => {
                let storage = self.new_hidden_local(result.storage_type().clone())?;
                let signature = self.call_targets.indirect_result_signatures.alloc(
                    lir::IndirectResultCallSignature::scoop_sret(
                        arguments,
                        result.clone(),
                        calling_convention,
                    ),
                );
                (
                    PendingTypedCall::IndirectResult {
                        signature,
                        storage,
                        args: lowered_args,
                    },
                    Some(lir::Value::Local(storage)),
                )
            }
        })
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
                    safepoint: self.new_safepoint(lir::SafepointSiteRole::ManagedCall),
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
    ) -> StorageResult<lir::CallSite> {
        Ok(match destination {
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
                    safepoint: self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition),
                    roots: lir::NativeSafeRootSet::default(),
                })
            }
            NativeCallDestination::Borrowed(destination) => {
                let result_scan = call.result_scan(&self.call_targets).clone();
                let result = match lir::NonEmptyRefScan::new(result_scan) {
                    None => lir::NativeBorrowedResultRoot::GcFree,
                    Some(_) => {
                        let storage = match &call {
                            PendingTypedCall::Void { .. } | PendingTypedCall::ElidedZst { .. } => {
                                unreachable!("void native call cannot have a result root")
                            }
                            PendingTypedCall::Direct { signature, .. } => {
                                let ty = self.call_targets.direct_signatures[*signature]
                                    .result()
                                    .storage_type()
                                    .clone();
                                self.new_hidden_local(ty)?
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
                    safepoint: self.new_safepoint(lir::SafepointSiteRole::NativeBorrowedTransition),
                    roots: lir::NativeBorrowedRootSet::default(),
                })
            }
        })
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
                    safepoint: self.new_safepoint(lir::SafepointSiteRole::ManagedInvoke),
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
}
