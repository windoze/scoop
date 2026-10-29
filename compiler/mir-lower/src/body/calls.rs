use super::*;

impl BodyLowerer<'_> {
    pub(super) fn record_suspend_function_call(&mut self, function: hir::FunctionId) {
        self.contains_suspend_call |= self.module.functions[function].is_suspend;
    }

    /// Option tests and guarded payload projections are tied to one immutable
    /// local identity by the MIR contract. HIR safe-call/Elvis desugaring
    /// supplies that shared hidden local; a standalone compiler-generated test
    /// is stabilized here without evaluating its operand twice.
    pub(super) fn lower_stable_option_operand(&mut self, operand: &hir::Expr) -> smir::Expr {
        let value = self.lower_expr(operand);
        if let smir::ExprKind::Local(local) = value.kind
            && !self.locals[local].mutable
        {
            return value;
        }
        let ty = value.ty.clone();
        let local = self.new_hidden("opt", ty.clone(), false);
        self.prelude
            .push(smir::StatementKind::ValDecl { local, init: value });
        smir::Expr::local(local, ty)
    }

    pub(super) fn lower_direct_super_method_call(
        &mut self,
        receiver: &hir::Expr,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = self.module.callable_function(callable);
        self.record_suspend_function_call(function);
        let callee = self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        );
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(self.lower_expr(receiver));
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        let return_ty = self.lower_type(result_ty);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget::Direct(callee),
                args: call_args,
                return_ty,
            }),
        )
    }

    /// `x!!`: the operand is evaluated once into a hidden local, then
    /// `if (VariantTest(Some)) { val $uw = VariantPayloadProject(Some._1) }
    /// else { throw UnwrapException() }`.
    /// The if/else is queued in `prelude` — it must precede the
    /// statement this expression belongs to — and the expression
    /// itself becomes the result local. The exception is an ordinary
    /// constructor call (`throw_builtin`, M8).
    pub(super) fn trapping_unwrap(
        &mut self,
        operand: &hir::Expr,
        result_ty: hir::TypeId,
        span: Span,
        some: OptionSomeRefs,
    ) -> smir::Expr {
        let option_ty = self.lower_type(operand.ty);
        let payload_ty = self.lower_type(result_ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("opt", option_ty.clone(), false);
        let result = self.new_hidden("uw", payload_ty.clone(), false);
        let throw = match self.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                self.throw_builtin(protocols.exceptions.unwrap_exception, span)
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => self.throw_imported_exception(
                protocols.exceptions().unwrap_exception().persistent(),
                protocols.exceptions().unwrap_exception_constructor(),
                span,
            ),
        };
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        let payload = smir::Expr::variant_payload_project(
            &self.enums.defs,
            smir::Expr::local(slot, option_ty.clone()),
            some.payload,
        );
        assert_eq!(
            payload.ty, payload_ty,
            "the checked Option payload is the HIR unwrap result type"
        );
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::variant_test(
                &self.enums.defs,
                smir::Expr::local(slot, option_ty.clone()),
                some.variant,
            ),
            then_body: vec![smir::Statement {
                debug_origin: Default::default(),
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: payload,
                },
                span,
            }],
            else_body: Some(vec![throw]),
        });
        smir::Expr::local(result, payload_ty)
    }

    pub(super) fn lower_call(
        &mut self,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = self.module.callable_function(callable);
        self.record_suspend_function_call(function);
        if let Some(protocol) = self
            .module
            .coroutine_protocol_for_function(function)
            .copied()
        {
            if function == protocol.start_coroutine {
                return self.lower_coroutine_start(protocol, args);
            }
            return self.lower_coroutine_suspend(protocol, args);
        }
        // `@Intrinsic` primitive functions (scoop.core, M7 DESIGN
        // section 2): handled up front — generic intrinsics (the M9
        // GC facilities) take this path too, before the generic-callee
        // arm below would reject their missing function-map entry.
        if let hir::FunctionKind::Intrinsic(intrinsic) = &self.module.functions[function].kind {
            return self.lower_intrinsic_call(intrinsic.kind, args, result_ty);
        }
        let callee = self.lower_user_callee(callable);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    pub(super) fn lower_user_callee(&mut self, callable: hir::Callable) -> mir::Callee {
        let function = self.module.callable_function(callable);
        match &self.module.functions[function].kind {
            hir::FunctionKind::User(_)
            | hir::FunctionKind::Abstract { .. }
            | hir::FunctionKind::InitializationEnsure => self.instances.get(function).map_or_else(
                || mir::Callee::User(self.function_map[&function]),
                mir::Callee::Monomorphized,
            ),
            hir::FunctionKind::Extern(extern_id) => {
                // A specialization has the same call boundary in its provider
                // and consumers, including generated closures in its body.
                let managed_caller = match self.current_owner {
                    crate::local_values::BodyOwner::Emitted(mir::LocalValueOwner::Function(
                        caller,
                    )) => self.functions[caller].gc_effect == mir::GcEffect::Managed,
                    crate::local_values::BodyOwner::Emitted(mir::LocalValueOwner::ReleaseHook(
                        _,
                    )) => false,
                    crate::local_values::BodyOwner::Imported(callable) => {
                        self.external_callables[callable].gc_effect() == mir::GcEffect::Managed
                    }
                };
                if managed_caller
                    && !matches!(
                        self.current_materialization.context(),
                        hir::CallableMaterializationContext::NoSubstitution
                    )
                {
                    mir::Callee::User(self.function_map[&function])
                } else {
                    mir::Callee::Extern(self.extern_map[extern_id])
                }
            }
            hir::FunctionKind::Intrinsic(_) => unreachable!("handled above"),
        }
    }

    /// An `@Intrinsic` call: the typed intrinsic kind maps directly onto the
    /// runtime function (`print` / `println` themselves are ordinary
    /// overloaded core functions and take the `User` path in
    /// `lower_call`). Raw intrinsic names do not reach this stage.
    ///
    /// The M9 GC facilities (milestone9 DESIGN section 1, runtime spec
    /// 3.4) marshal between the raw machine word the runtime functions
    /// speak (`u64` addresses / handle values) and the Scoop-level
    /// handle aggregates: `pin` / `getGcHandle` wrap the word into the
    /// handle struct, `unpin` / `releaseGcHandle` unwrap it.
    pub(super) fn lower_intrinsic_call(
        &mut self,
        kind: hir::IntrinsicFunctionKind,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = match kind {
            hir::IntrinsicFunctionKind::String(kind) => match kind {
                hir::StringIntrinsic::ByteLength => mir::RuntimeFn::StringByteLength,
                hir::StringIntrinsic::ByteAt => mir::RuntimeFn::StringByteAt,
                hir::StringIntrinsic::CharacterAtByte => mir::RuntimeFn::StringCharacterAtByte,
                hir::StringIntrinsic::ByteAtUnchecked | hir::StringIntrinsic::SetByteUnchecked => {
                    return self.lower_string_byte(kind, args, result_ty);
                }
                hir::StringIntrinsic::CopyUtf8Unchecked
                | hir::StringIntrinsic::AllocateZeroed
                | hir::StringIntrinsic::SliceBytes
                | hir::StringIntrinsic::FromBytes
                | hir::StringIntrinsic::ToBytes
                | hir::StringIntrinsic::ToMutableBytes
                | hir::StringIntrinsic::FromBytePrefixUnchecked
                | hir::StringIntrinsic::CopyToBytesUnchecked
                | hir::StringIntrinsic::CopyBytePrefixUnchecked
                | hir::StringIntrinsic::CopyRawBytesUnchecked => {
                    return self.lower_string_copy(kind, args, result_ty);
                }
            },
            hir::IntrinsicFunctionKind::MaybeUninit(kind) => {
                return self.lower_maybe_uninit(kind, args.first(), result_ty);
            }
            hir::IntrinsicFunctionKind::GcPinRaw => mir::RuntimeFn::Pin,
            hir::IntrinsicFunctionKind::GcUnpinRaw => mir::RuntimeFn::Unpin,
            hir::IntrinsicFunctionKind::GcGetHandleRaw => mir::RuntimeFn::GetHandle,
            hir::IntrinsicFunctionKind::GcReleaseHandleRaw => mir::RuntimeFn::ReleaseHandle,
            hir::IntrinsicFunctionKind::GcCollect => mir::RuntimeFn::GcCollect,
            hir::IntrinsicFunctionKind::GcStats => mir::RuntimeFn::GcStats,
            hir::IntrinsicFunctionKind::ThrowablePrintStackTrace => {
                mir::RuntimeFn::ThrowablePrintStackTrace
            }
            hir::IntrinsicFunctionKind::CoroutineStart
            | hir::IntrinsicFunctionKind::CoroutineSuspend => {
                unreachable!("coroutine intrinsics are lowered through the typed protocol")
            }
            hir::IntrinsicFunctionKind::DataBorrow(_)
            | hir::IntrinsicFunctionKind::Float(_)
            | hir::IntrinsicFunctionKind::Char(_)
            | hir::IntrinsicFunctionKind::Integer(_)
            | hir::IntrinsicFunctionKind::Array(_)
            | hir::IntrinsicFunctionKind::Atomic(_)
            | hir::IntrinsicFunctionKind::ArrayAccess(_)
            | hir::IntrinsicFunctionKind::PrimitiveUnary(_)
            | hir::IntrinsicFunctionKind::PrimitiveBinary(_)
            | hir::IntrinsicFunctionKind::Pointer(_)
            | hir::IntrinsicFunctionKind::CurrentSourceLocation
            | hir::IntrinsicFunctionKind::TypeReference
            | hir::IntrinsicFunctionKind::TypeDescription
            | hir::IntrinsicFunctionKind::AnnotationKey
            | hir::IntrinsicFunctionKind::ForeignCallbackRegister
            | hir::IntrinsicFunctionKind::ForeignCallbackRetain
            | hir::IntrinsicFunctionKind::ForeignCallbackRelease
            | hir::IntrinsicFunctionKind::ForeignCallbackState
            | hir::IntrinsicFunctionKind::ForeignCallbackFailure => {
                unreachable!("HIR expands this intrinsic before MIR")
            }
        };
        let callee = mir::Callee::Runtime(function);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    pub(super) fn call(
        &mut self,
        callee: mir::Callee,
        args: &[&hir::Expr],
        return_ty: mir::Type,
    ) -> smir::Expr {
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget::Direct(callee),
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                return_ty,
            }),
        )
    }

    /// A resolved method call (impl spec 2.9): the receiver becomes
    /// argument 0 (`this`), and the call kind is annotated from the
    /// receiver's static type — class receiver → `Virtual` (the M6
    /// simplification: class methods always dispatch through the
    /// vtable), interface receiver → `Interface` (the slot is the
    /// method signature's index in the interface declaration), value
    /// type → `Direct`. A method without a vtable slot (generic
    /// methods never enter the vtable) stays `Direct`. The slot is
    /// located by the callee's signature (`signature_key`), so
    /// overloads dispatch to their own slot and overrides hit the
    /// replaced base slot.
    pub(super) fn lower_method_call(
        &mut self,
        receiver: &hir::Expr,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let module = self.module;
        let function = module.callable_function(callable);
        self.record_suspend_function_call(function);
        let f = &module.functions[function];
        if let hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::MaybeUninit(kind),
            ..
        }) = &f.kind
        {
            let operand = if *kind == hir::MaybeUninitIntrinsic::AssumeInit {
                Some(receiver)
            } else {
                let receiver = self.lower_expr(receiver);
                self.prelude.push(smir::StatementKind::Expr(receiver));
                args.first()
            };
            return self.lower_maybe_uninit(*kind, operand, result_ty);
        }
        if let hir::FunctionKind::Intrinsic(intrinsic) = &f.kind
            && let Some(operation) =
                crate::closures::primitive::PrimitiveReference::from_intrinsic(intrinsic.kind)
        {
            // A bound call can select a concrete intrinsic during monomorphization.
            // Preserve its direct operation instead of calling its itable entry.
            let receiver = self.lower_expr(receiver);
            let result = self.lower_type(result_ty);
            return match operation {
                crate::closures::primitive::PrimitiveReference::Unary(operation) => {
                    self.unary_reference_value(operation, receiver, &result)
                }
                crate::closures::primitive::PrimitiveReference::Binary(operation) => {
                    let [rhs] = args else {
                        unreachable!("a binary intrinsic has one explicit operand")
                    };
                    let right = self.lower_expr(rhs);
                    self.binary_reference_value(operation, receiver, right, &result, rhs.span)
                }
            };
        }
        let callee = self.instances.get(function).map_or_else(
            || mir::DispatchCallee::User(self.function_map[&function]),
            mir::DispatchCallee::Monomorphized,
        );
        // The receiver's static type decides the dispatch kind.
        let receiver_class = match &module.types[receiver.ty].kind {
            hir::TypeKind::Class(class) => Some(*class),
            hir::TypeKind::Interface(..) => None,
            hir::TypeKind::Any => unreachable!("Any has no methods"),
            _ => None,
        };
        let dispatch = f
            .receiver
            .method()
            .expect("a method call names method metadata")
            .dispatch;
        let receiver = self.lower_expr(receiver);
        let mut call_args = Vec::with_capacity(args.len() + 1);
        let target = match dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                call_args.push(receiver);
                mir::CallTarget::Direct(callee.into())
            }
            hir::MethodDispatch::Virtual(family) => {
                let class =
                    receiver_class.expect("a virtual family is called through a class receiver");
                let slot = self.method_slots[&self.class_map[&class]][&family];
                mir::CallTarget::Virtual {
                    callee,
                    slot,
                    receiver: Box::new(receiver),
                }
            }
            hir::MethodDispatch::Interface { interface, slot } => {
                let interface = self.interfaces.mir_id(interface);
                mir::CallTarget::Interface {
                    callee,
                    interface,
                    slot: slot.into_raw(),
                    receiver: Box::new(receiver),
                }
            }
        };
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        let return_ty = self.lower_type(result_ty);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target,
                args: call_args,
                return_ty,
            }),
        )
    }
}
