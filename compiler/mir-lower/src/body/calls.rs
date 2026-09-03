use super::*;

impl BodyLowerer<'_> {
    /// `x!!`: the operand is evaluated once into a hidden local, then
    /// `if (tag == Some) { val $uw = <field 0> } else { throw UnwrapException() }`.
    /// The if/else is queued in `prelude` — it must precede the
    /// statement this expression belongs to — and the expression
    /// itself becomes the result local. The exception is an ordinary
    /// constructor call (`throw_builtin`, M8).
    pub(super) fn trapping_unwrap(
        &mut self,
        operand: &hir::Expr,
        result_ty: hir::TypeId,
        span: Span,
        some: u32,
    ) -> smir::Expr {
        let option_ty = self.lower_type(operand.ty);
        let payload_ty = self.lower_type(result_ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("opt", option_ty.clone(), false);
        let result = self.new_hidden("uw", payload_ty.clone(), false);
        let throw = self.throw_builtin(self.module.exception_core.unwrap_exception, span);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(smir::Expr::local(
                            slot,
                            option_ty.clone(),
                        ))),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                },
            ),
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: smir::Expr::new(
                        payload_ty.clone(),
                        smir::ExprKind::EnumField {
                            operand: Box::new(smir::Expr::local(slot, option_ty)),
                            variant: some,
                            index: 0,
                        },
                    ),
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
            hir::FunctionKind::User(_) => self.instances.get(function).map_or_else(
                || mir::Callee::User(self.function_map[&function]),
                mir::Callee::Monomorphized,
            ),
            hir::FunctionKind::Extern(extern_id) => mir::Callee::Extern(self.extern_map[extern_id]),
            hir::FunctionKind::Intrinsic(_) => unreachable!("handled above"),
        }
    }

    pub(super) fn lower_coroutine_start(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [task, completion] = args else {
            unreachable!("hir-lower validates startCoroutine's two parameters")
        };
        let result = self.lower_type(protocol.result_type);
        let task = self.lower_expr(task);
        let completion = self.lower_expr(completion);
        let task_interface = self.interfaces.mir_id(protocol.suspend_task);
        let continuation_interface = self.interfaces.mir_id(protocol.continuation);
        let (_, step_ty) = self
            .coroutines
            .step_for(&result, self.structs, self.enums, self.shell);
        let run = self.instances.get(protocol.suspend_task_run).unwrap();
        let resume = self.instances.get(protocol.continuation_resume).unwrap();
        let failure = self
            .instances
            .get(protocol.continuation_resume_with_exception)
            .unwrap();
        let throwable =
            mir::Type::Class(self.class_map[&self.module.exception_core.throwable.class()]);
        let helper = self.coroutines.start_helper(
            &result,
            task_interface,
            continuation_interface,
            run,
            resume,
            failure,
            &step_ty,
            throwable,
            self.functions,
            self.top_level,
            self.shell,
        );
        self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
            mir::Type::Unit,
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(helper),
                },
                args: vec![task, completion],
                return_ty: mir::Type::Unit,
            }),
        )));
        smir::Expr::unit()
    }

    pub(super) fn lower_coroutine_suspend(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [registration] = args else {
            unreachable!("hir-lower validates suspendCoroutine's one parameter")
        };
        let result = self.lower_type(protocol.result_type);
        let registration_interface = self.interfaces.mir_id(protocol.suspend_registration);
        let register = self
            .instances
            .get(protocol.suspend_registration_register)
            .unwrap();
        smir::Expr::new(
            result.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: registration_interface,
                        slot: 0,
                    },
                    callee: mir::Callee::CoroutineSuspend { register },
                },
                args: vec![self.lower_expr(registration)],
                return_ty: result,
            }),
        )
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
            hir::IntrinsicFunctionKind::GcPinRaw => mir::RuntimeFn::Pin,
            hir::IntrinsicFunctionKind::GcUnpinRaw => mir::RuntimeFn::Unpin,
            hir::IntrinsicFunctionKind::GcGetHandleRaw => mir::RuntimeFn::GetHandle,
            hir::IntrinsicFunctionKind::GcReleaseHandleRaw => mir::RuntimeFn::ReleaseHandle,
            hir::IntrinsicFunctionKind::GcCollect => mir::RuntimeFn::GcCollect,
            hir::IntrinsicFunctionKind::GcStats => mir::RuntimeFn::GcStats,
            hir::IntrinsicFunctionKind::CoroutineStart
            | hir::IntrinsicFunctionKind::CoroutineSuspend => {
                unreachable!("coroutine intrinsics are lowered through the typed protocol")
            }
            hir::IntrinsicFunctionKind::Array(_)
            | hir::IntrinsicFunctionKind::ArrayAccess(_)
            | hir::IntrinsicFunctionKind::PrimitiveUnary(_)
            | hir::IntrinsicFunctionKind::PrimitiveBinary(_)
            | hir::IntrinsicFunctionKind::Pointer(_)
            | hir::IntrinsicFunctionKind::CurrentSourceLocation
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
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
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
        let f = &module.functions[function];
        let callee = self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        );
        // The receiver's static type decides the dispatch kind.
        let receiver_class = match &module.types[receiver.ty].kind {
            hir::TypeKind::Class(class) => Some(*class),
            hir::TypeKind::Interface(..) => None,
            hir::TypeKind::Any => unreachable!("Any has no methods"),
            _ => None,
        };
        let dispatch = f
            .method
            .expect("a method call names method metadata")
            .dispatch;
        let kind = match dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                mir::CallKind::Direct
            }
            hir::MethodDispatch::Virtual(family) => {
                let class =
                    receiver_class.expect("a virtual family is called through a class receiver");
                let slot = self.method_slots[&self.class_map[&class]][&family];
                mir::CallKind::Virtual { slot }
            }
            hir::MethodDispatch::Interface { interface, slot } => {
                let interface = self.interfaces.mir_id(interface);
                mir::CallKind::Interface {
                    interface,
                    slot: slot.into_raw(),
                }
            }
        };
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(self.lower_expr(receiver));
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        let return_ty = self.lower_type(result_ty);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget { kind, callee },
                args: call_args,
                return_ty,
            }),
        )
    }
}
