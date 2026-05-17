//! FnLowering call lowering: arg canonicalization, direct/dispatch/intrinsic/funptr call variants.

#![allow(dead_code)]

use super::*;

impl<'a> FnLowering<'a> {
    pub(in crate::mir::lower) fn source_arg_expected_tys_for_callee_ty(
        &self,
        callee_ty: TypeId,
        explicit_arg_count: usize,
        binding: Option<&CallArgBindingContract>,
    ) -> Vec<Option<TypeId>> {
        let TypeKind::Ref(RefTypeKind::Function(fun)) = self.types.kind(callee_ty) else {
            return vec![None; explicit_arg_count];
        };
        let mut param_tys =
            Vec::with_capacity(fun.params.len() + usize::from(fun.receiver.is_some()));
        if let Some(receiver_ty) = fun.receiver {
            param_tys.push(receiver_ty);
        }
        param_tys.extend(fun.params.iter().copied());
        self.source_arg_expected_tys_from_param_tys(&param_tys, explicit_arg_count, false, binding)
    }

    pub(in crate::mir::lower) fn lower_typed_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> bool {
        let Some(contract) = self
            .facts
            .call_site_contract(self.source_path.as_path(), span)
            .cloned()
        else {
            return false;
        };
        if !self.typed_call_contract_matches_callee(&contract, callee) {
            return false;
        }

        match contract {
            TypedCallSiteContract::DirectTopLevel(function) => {
                self.lower_direct_call_expr(span, result, function.fqn(), args, Some(&function));
                true
            }
            TypedCallSiteContract::MemberDirect(member) => {
                self.lower_direct_call_expr(
                    span,
                    result,
                    member.function().fqn(),
                    args,
                    Some(member.function()),
                );
                true
            }
            TypedCallSiteContract::Extension { function, .. } => {
                self.lower_direct_call_expr(span, result, function.fqn(), args, Some(&function));
                true
            }
            TypedCallSiteContract::Constructor(ctor) => {
                self.lower_constructor_call_expr(span, result, &ctor, args);
                true
            }
            TypedCallSiteContract::Closure { arg_binding, .. } => {
                self.lower_callable_value_call_expr(
                    span,
                    result,
                    callee,
                    args,
                    arg_binding.as_ref(),
                    true,
                );
                true
            }
            TypedCallSiteContract::FunValue { arg_binding, .. } => {
                self.lower_callable_value_call_expr(
                    span,
                    result,
                    callee,
                    args,
                    arg_binding.as_ref(),
                    false,
                );
                true
            }
            TypedCallSiteContract::FunPtr { arg_binding, .. } => {
                self.lower_funptr_call_expr(span, result, callee, args, arg_binding.as_ref());
                true
            }
            TypedCallSiteContract::Virtual(member) => {
                self.lower_dispatch_call_expr_from_contract(
                    span,
                    result,
                    callee,
                    args,
                    DispatchTargetKind::Virtual,
                    &member,
                );
                true
            }
            TypedCallSiteContract::Interface(member) => {
                self.lower_dispatch_call_expr_from_contract(
                    span,
                    result,
                    callee,
                    args,
                    DispatchTargetKind::Interface,
                    &member,
                );
                true
            }
            TypedCallSiteContract::Intrinsic { kind, function } => {
                self.lower_intrinsic_call_expr(span, result, &kind, function.fqn(), args)
            }
            TypedCallSiteContract::EffectOp(_) | TypedCallSiteContract::ContinuationResume(_) => {
                false
            }
        }
    }

    pub(in crate::mir::lower) fn typed_call_contract_matches_callee(
        &self,
        contract: &TypedCallSiteContract,
        callee: &hir::Expr,
    ) -> bool {
        let Some(callee_fqn) = top_level_callee_fqn(callee) else {
            return true;
        };
        let contract_fqn = match contract {
            TypedCallSiteContract::DirectTopLevel(function) => function.fqn(),
            TypedCallSiteContract::MemberDirect(member) => member.function().fqn(),
            TypedCallSiteContract::Extension { function, .. }
            | TypedCallSiteContract::Intrinsic { function, .. } => function.fqn(),
            TypedCallSiteContract::Constructor(_)
            | TypedCallSiteContract::Closure { .. }
            | TypedCallSiteContract::FunValue { .. }
            | TypedCallSiteContract::FunPtr { .. }
            | TypedCallSiteContract::Virtual(_)
            | TypedCallSiteContract::Interface(_)
            | TypedCallSiteContract::EffectOp(_)
            | TypedCallSiteContract::ContinuationResume(_) => return true,
        };
        intrinsic_base_fqn(contract_fqn) == intrinsic_base_fqn(callee_fqn)
    }

    pub(in crate::mir::lower) fn lower_direct_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee_fqn: &str,
        args: &[hir::CallArg],
        function: Option<&FunctionTargetContract>,
    ) {
        let arg_binding = function
            .and_then(FunctionTargetContract::arg_binding)
            .filter(|binding| !call_arg_binding_has_receiver(binding));
        let arg_binding = Self::active_hir_call_arg_binding(args, arg_binding);
        let expected_tys = function
            .and_then(|function| self.top_level_fun_param_tys.get(function.fqn()))
            .map(|param_tys| {
                self.source_arg_expected_tys_from_param_tys(
                    param_tys,
                    args.len(),
                    true,
                    arg_binding,
                )
            })
            .unwrap_or_else(|| vec![None; args.len()]);
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return;
        };
        let args = self.canonicalize_call_args_from_binding(args, arg_binding);
        let kind = CallKind::Direct {
            callee_fqn: callee_fqn.to_string(),
        };
        let terminates_current_block = matches!(
            &kind,
            CallKind::Direct { callee_fqn } if callee_fqn == "scoop.core.panic"
        );
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
        if terminates_current_block {
            self.set_terminator(self.current_bb, span, TerminatorKind::Unreachable);
        }
    }

    pub(in crate::mir::lower) fn lower_constructor_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        ctor: &crate::pipeline::ConstructorCallTargetContract,
        args: &[hir::CallArg],
    ) {
        let Some(args) = self.lower_call_args(args) else {
            return;
        };
        let hidden_effects = self
            .facts
            .class_ctor_hidden_effects(self.source_path.as_path(), span);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::ClassCtor {
                site_id,
                class_fqn: ctor.owner_fqn().to_string(),
                ctor: ClassCtorCallMetadata {
                    selected_ctor_span: ctor.ctor_span(),
                    ordered_param_count: ctor.arg_mapping().len(),
                },
                args,
                hidden_effects,
            },
        );
    }

    pub(in crate::mir::lower) fn lower_callable_value_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        arg_binding: Option<&CallArgBindingContract>,
        prefer_closure_kind: bool,
    ) {
        let callee_local = self.lower_expr_to_local(callee);
        if self.current_is_terminated() {
            return;
        }
        let callee_ty = self.body.locals[callee_local.as_u32() as usize].ty;
        let arg_binding = Self::active_hir_call_arg_binding(args, arg_binding);
        let expected_tys =
            self.source_arg_expected_tys_for_callee_ty(callee_ty, args.len(), arg_binding);
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return;
        };
        let args = self.canonicalize_call_args_from_binding(args, arg_binding);
        let origin = self.value_origins.get(&callee_local).cloned();
        let gc_intrinsic_callee =
            gc_intrinsic_callee_from_origin(origin.as_ref()).map(str::to_string);
        let kind = match (prefer_closure_kind, origin) {
            (true, Some(ValueOrigin::Closure { fn_ptr })) => CallKind::Closure {
                callee: Operand::Local(callee_local),
                fn_ptr,
            },
            _ => CallKind::FunValue {
                callee: Operand::Local(callee_local),
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            gc_intrinsic_callee.as_deref(),
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    pub(in crate::mir::lower) fn lower_funptr_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        arg_binding: Option<&CallArgBindingContract>,
    ) {
        let callee_local = self.lower_expr_to_local(callee);
        if self.current_is_terminated() {
            return;
        }
        let callee_ty = self.body.locals[callee_local.as_u32() as usize].ty;
        let arg_binding = Self::active_hir_call_arg_binding(args, arg_binding);
        let expected_tys =
            self.source_arg_expected_tys_for_callee_ty(callee_ty, args.len(), arg_binding);
        let Some(args) = self.lower_call_args_with_expected(args, &expected_tys) else {
            return;
        };
        let args = self.canonicalize_call_args_from_binding(args, arg_binding);
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &CallKind::FunPtr {
                callee: Operand::Local(callee_local),
            },
            &args,
            None,
        );
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind: CallKind::FunPtr {
                    callee: Operand::Local(callee_local),
                },
                args,
                transport,
            },
        );
    }

    pub(in crate::mir::lower) fn lower_intrinsic_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        kind: &TypedIntrinsicKind,
        callee_fqn: &str,
        args: &[hir::CallArg],
    ) -> bool {
        let intrinsic_fqn = intrinsic_base_fqn(callee_fqn);
        match (kind, intrinsic_fqn) {
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.sizeOf") if name == "sizeOf" => {
                let value_ty = args
                    .first()
                    .map(|arg| match arg {
                        hir::CallArg::Positional(value) => value.ty,
                        hir::CallArg::Named { value, .. } => value.ty,
                    })
                    .or_else(|| {
                        self.facts
                            .call_site_contract(self.source_path.as_path(), span)
                            .and_then(|contract| match contract {
                                TypedCallSiteContract::Intrinsic { function, .. } => {
                                    function.type_args().first().copied()
                                }
                                _ => None,
                            })
                    })
                    .expect("typed sizeOf intrinsic must publish a value or type argument");
                self.assign(span, result, Rvalue::SizeOf { value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.alignOf")
                if name == "alignOf" =>
            {
                let value_ty = self.reflection_type_arg_for_call(span, "alignOf");
                self.assign(span, result, Rvalue::AlignOf { value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.nameOf") if name == "nameOf" => {
                let source_ty = self
                    .facts
                    .call_site_contract(self.source_path.as_path(), span)
                    .and_then(|contract| match contract {
                        TypedCallSiteContract::Intrinsic { function, .. } => {
                            function.type_args().first().copied()
                        }
                        _ => None,
                    })
                    .expect("typed nameOf intrinsic must publish a type argument");
                self.assign(
                    span,
                    result,
                    Rvalue::TypeMetadataLiteral(TypeMetadataLiteral {
                        source_ty,
                        source_fqn: self.nominal_fqn_for_ty(source_ty),
                        kind: TypeMetadataLiteralKind::TypeNameString,
                    }),
                );
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.kindOf") if name == "kindOf" => {
                let value_ty = self.reflection_type_arg_for_call(span, "kindOf");
                self.assign(span, result, Rvalue::KindOf { value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.descOf") if name == "descOf" => {
                let value_ty = self.reflection_type_arg_for_call(span, "descOf");
                self.assign(span, result, Rvalue::DescOf { value_ty });
                true
            }
            _ => {
                self.lower_direct_call_expr(span, result, callee_fqn, args, None);
                true
            }
        }
    }

    pub(in crate::mir::lower) fn lower_dispatch_call_expr_from_contract(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        dispatch_kind: DispatchTargetKind,
        member: &MemberCallTargetContract,
    ) {
        let (receiver_expr, call_args) = self.dispatch_receiver_and_args(callee, args);
        let receiver_local = self.lower_expr_to_local(receiver_expr);
        if self.current_is_terminated() {
            return;
        }
        let stripped_binding = call_arg_binding_without_receiver(member.function().arg_binding());
        let arg_binding = Self::active_hir_call_arg_binding(call_args, stripped_binding.as_ref());
        let function_has_receiver = member
            .function()
            .arg_binding()
            .is_some_and(call_arg_binding_has_receiver);
        let expected_tys = self
            .top_level_fun_param_tys
            .get(member.function().fqn())
            .map(|param_tys| {
                let param_tys = if function_has_receiver {
                    param_tys.get(1..).unwrap_or(&[])
                } else {
                    param_tys.as_slice()
                };
                self.source_arg_expected_tys_from_param_tys(
                    param_tys,
                    call_args.len(),
                    false,
                    arg_binding,
                )
            })
            .unwrap_or_else(|| vec![None; call_args.len()]);
        let Some(args) = self.lower_call_args_with_expected(call_args, &expected_tys) else {
            return;
        };
        let dispatch = DispatchMetadata {
            owner_fqn: member.owner_fqn().to_string(),
            member_name: member.member_name().to_string(),
            member_fqn: member.member_fqn().to_string(),
            member_decl_span: member.function().decl_span(),
            receiver_ty: member.receiver_ty(),
        };
        let kind = match dispatch_kind {
            DispatchTargetKind::Virtual => CallKind::Virtual {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
            DispatchTargetKind::Interface => CallKind::Interface {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
        };
        let site_id = self.fresh_site_id();
        let transport = self.call_transport_metadata(
            self.body.locals[result.as_u32() as usize].ty,
            &kind,
            &args,
            None,
        );
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
                transport,
            },
        );
    }

    pub(in crate::mir::lower) fn dispatch_receiver_and_args<'b>(
        &self,
        callee: &'b hir::Expr,
        args: &'b [hir::CallArg],
    ) -> (&'b hir::Expr, &'b [hir::CallArg]) {
        match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                let Some((receiver_arg, remaining_args)) = args.split_first() else {
                    panic!("typed dispatch call contract must include a receiver argument")
                };
                let receiver_expr = match receiver_arg {
                    hir::CallArg::Positional(expr) => expr,
                    hir::CallArg::Named { value, .. } => value,
                };
                (receiver_expr, remaining_args)
            }
            hir::ExprKind::MemberAccess { receiver, .. } => (receiver.as_ref(), args),
            _ => panic!("typed dispatch call contract must match a dispatch callee shape"),
        }
    }

    pub(in crate::mir::lower) fn nominal_fqn_for_ty(&self, ty: TypeId) -> Option<String> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.clone()),
            _ => None,
        }
    }

    fn reflection_type_arg_for_call(&self, span: Span, name: &str) -> TypeId {
        self.facts
            .call_site_contract(self.source_path.as_path(), span)
            .and_then(|contract| match contract {
                TypedCallSiteContract::Intrinsic { function, .. } => {
                    function.type_args().first().copied()
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("typed {name} intrinsic must publish a type argument"))
    }

    pub(in crate::mir::lower) fn operand_ty(&self, operand: &Operand) -> TypeId {
        match operand {
            Operand::Local(local) => self.body.locals[local.as_u32() as usize].ty,
            Operand::Const(ConstValue::Bool(_)) => self.builtins.bool_,
            Operand::Const(ConstValue::Char) => self.builtins.char_,
            Operand::Const(ConstValue::Unit) => self.builtins.unit,
            Operand::Const(ConstValue::Int) => self.builtins.int,
            Operand::Const(ConstValue::SynthInt(_)) => self.builtins.int,
            Operand::Const(ConstValue::Float64) => self.builtins.float64,
            Operand::Const(ConstValue::Float32) => self.builtins.float32,
            Operand::Const(ConstValue::String) => self.builtins.string,
        }
    }

    pub(in crate::mir::lower) fn transport_kind_for_ty(&self, ty: TypeId) -> MirTransportKind {
        mir_transport_kind_for_ty(self.types, self.facts, ty)
    }

    pub(in crate::mir::lower) fn is_aggregate_transport_ty(&self, ty: TypeId) -> bool {
        mir_is_aggregate_transport_ty(self.types, ty)
    }

    pub(in crate::mir::lower) fn transport_requirements(
        &self,
        ty: TypeId,
    ) -> MirTransportRequirements {
        mir_transport_requirements(self.types, ty)
    }

    pub(in crate::mir::lower) fn value_transport_with_kind(
        &self,
        ty: TypeId,
        kind: MirTransportKind,
    ) -> ValueTransportMetadata {
        ValueTransportMetadata {
            source_ty: ty,
            kind,
            requirements: self.transport_requirements(ty),
            boxing: None,
        }
    }

    pub(in crate::mir::lower) fn value_transport(&self, ty: TypeId) -> ValueTransportMetadata {
        self.value_transport_with_kind(ty, self.transport_kind_for_ty(ty))
    }

    pub(in crate::mir::lower) fn value_transport_with_boxing_reason(
        &self,
        ty: TypeId,
        kind: MirTransportKind,
        reason: MirBoxingReason,
        target_ty: Option<TypeId>,
    ) -> ValueTransportMetadata {
        let mut transport = self.value_transport_with_kind(ty, kind);
        if self.is_aggregate_transport_ty(ty) {
            transport.boxing = Some(MirBoxingIntent {
                source_ty: ty,
                target_ty,
                reason,
            });
        }
        transport
    }

    pub(in crate::mir::lower) fn aggregate_transport(
        &self,
        aggregate_ty: TypeId,
        kind: AggregateTransportKind,
        fields: impl IntoIterator<Item = (Option<String>, TypeId)>,
    ) -> AggregateTransportMetadata {
        AggregateTransportMetadata {
            aggregate_ty,
            kind,
            fields: fields
                .into_iter()
                .enumerate()
                .map(|(index, (name, ty))| AggregateTransportField {
                    index,
                    name,
                    ty,
                    transport: self.value_transport(ty),
                })
                .collect(),
        }
    }

    pub(in crate::mir::lower) fn capture_box_contract(
        &self,
        box_ty: TypeId,
        inner_ty: TypeId,
    ) -> CaptureBoxTransportMetadata {
        CaptureBoxTransportMetadata {
            box_ty,
            value: self.value_transport_with_boxing_reason(
                inner_ty,
                self.transport_kind_for_ty(inner_ty),
                MirBoxingReason::ClosureCapture,
                Some(box_ty),
            ),
        }
    }

    pub(in crate::mir::lower) fn closure_env_contract(
        &self,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> ClosureEnvTransportMetadata {
        ClosureEnvTransportMetadata {
            env_ty,
            captures: captures
                .iter()
                .map(|capture| {
                    let kind = if capture.mutable {
                        MirTransportKind::CaptureBox
                    } else {
                        self.transport_kind_for_ty(capture.ty)
                    };
                    let transport = if capture.mutable {
                        self.value_transport_with_kind(capture.ty, kind)
                    } else {
                        self.value_transport_with_boxing_reason(
                            capture.ty,
                            kind,
                            MirBoxingReason::ClosureCapture,
                            Some(env_ty),
                        )
                    };
                    ClosureCaptureTransportMetadata {
                        name: capture.name.clone(),
                        decl_span: capture.decl_span,
                        mutable: capture.mutable,
                        source_local: capture.source_local,
                        transport,
                    }
                })
                .collect(),
        }
    }

    pub(in crate::mir::lower) fn call_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
        gc_intrinsic_callee: Option<&str>,
    ) -> CallTransportMetadata {
        let result = self.value_transport(result_ty);
        let aggregate_return = self
            .is_aggregate_transport_ty(result_ty)
            .then(|| result.clone());
        CallTransportMetadata {
            result,
            aggregate_return,
            array: self.array_transport_metadata(result_ty, kind, args),
            gc: self.gc_intrinsic_transport_metadata(result_ty, kind, args, gc_intrinsic_callee),
            thread_resume_payload: self.thread_resume_payload_transport_metadata(kind, args),
            abi: self.call_abi_handoff(kind),
        }
    }

    pub(in crate::mir::lower) fn thread_resume_payload_transport_metadata(
        &self,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Option<Box<ValueTransportMetadata>> {
        let CallKind::Direct { callee_fqn } = kind else {
            return None;
        };
        let base = intrinsic_base_fqn(callee_fqn);
        if !matches!(
            base,
            THREAD_SPAWN_JOIN_RESUME_FQN | THREAD_SPAWN_JOIN_RESUME_U64_FQN
        ) {
            return None;
        }
        let payload_ty = args
            .first()
            .map(|arg| self.operand_ty(&arg.value))
            .and_then(|ty| {
                continuation_contract_from_type(self.types, ty).map(|(resume, _, _)| resume)
            })
            .or_else(|| args.get(1).map(|arg| self.operand_ty(&arg.value)))?;
        Some(Box::new(self.value_transport_with_boxing_reason(
            payload_ty,
            MirTransportKind::EffectPayload,
            MirBoxingReason::EffectPayload,
            Some(payload_ty),
        )))
    }

    pub(in crate::mir::lower) fn gc_intrinsic_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
        gc_intrinsic_callee: Option<&str>,
    ) -> Option<GcIntrinsicTransportMetadata> {
        let callee_fqn = match gc_intrinsic_callee {
            Some(callee_fqn) => callee_fqn,
            None => match kind {
                CallKind::Direct { callee_fqn } => callee_fqn.as_str(),
                CallKind::Closure { .. }
                | CallKind::FunValue { .. }
                | CallKind::FunPtr { .. }
                | CallKind::Virtual { .. }
                | CallKind::Interface { .. }
                | CallKind::Resume { .. } => return None,
            },
        };
        let subject_ty = args
            .first()
            .map(|arg| self.operand_ty(&arg.value))
            .unwrap_or(self.builtins.any);
        let subject = self.value_transport(subject_ty);

        match callee_fqn {
            "scoop.core.GC.pin" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::Pin,
                root_lifetime: GcRootLifetime::PinnedUntilUnpin,
                pairing: GcIntrinsicPairing::PinMustPairUnpin,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(result_ty),
                subject,
            }),
            "scoop.core.GC.unpin" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::Unpin,
                root_lifetime: GcRootLifetime::EndsPinnedRoot,
                pairing: GcIntrinsicPairing::UnpinMatchesPin,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            "scoop.core.GC.handleNew" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleNew,
                root_lifetime: GcRootLifetime::StableHandleUntilDrop,
                pairing: GcIntrinsicPairing::HandleNewMustPairDrop,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(result_ty),
                subject,
            }),
            "scoop.core.GC.handleGet" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleGet,
                root_lifetime: GcRootLifetime::BorrowedFromStableHandle,
                pairing: GcIntrinsicPairing::HandleGetRequiresLiveHandle,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            "scoop.core.GC.handleDrop" => Some(GcIntrinsicTransportMetadata {
                callee_fqn: callee_fqn.to_string(),
                operation: GcIntrinsicOperation::HandleDrop,
                root_lifetime: GcRootLifetime::EndsStableHandle,
                pairing: GcIntrinsicPairing::HandleDropMatchesHandleNew,
                unsafe_required: true,
                subject_ty,
                token_ty: Some(subject_ty),
                subject,
            }),
            _ => None,
        }
    }

    pub(in crate::mir::lower) fn call_abi_handoff(
        &self,
        kind: &CallKind,
    ) -> CallAbiHandoffMetadata {
        match kind {
            CallKind::Direct { callee_fqn } if Self::is_plain_no_outward_intrinsic(callee_fqn) => {
                CallAbiHandoffMetadata::plain_no_outward()
            }
            _ => CallAbiHandoffMetadata::deferred_to_effect_facts(),
        }
    }

    pub(in crate::mir::lower) fn is_plain_no_outward_intrinsic(fqn: &str) -> bool {
        matches!(
            fqn,
            ARRAY_BUILDER_NEW_FQN
                | ARRAY_BUILDER_PUSH_FQN
                | ARRAY_BUILDER_PUSH_STRING_FQN
                | ARRAY_BUILDER_BUILD_ARRAY_FQN
                | ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN
                | ARRAY_BUILDER_BUILD_ARRAY_STRING_FQN
        )
    }

    pub(in crate::mir::lower) fn array_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Option<ArrayElementTransportMetadata> {
        let CallKind::Direct { callee_fqn } = kind else {
            return None;
        };
        match intrinsic_base_fqn(callee_fqn.as_str()) {
            ARRAY_BUILDER_PUSH_FQN | ARRAY_BUILDER_PUSH_STRING_FQN | "scoop.core.push" => {
                let builder_ty = args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                let element_ty = args
                    .get(1)
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderPush,
                    array_ty: builder_ty,
                    element_ty,
                    mutable: true,
                    element: self.value_transport_with_boxing_reason(
                        element_ty,
                        MirTransportKind::ArrayElement,
                        MirBoxingReason::ArrayElement,
                        Some(builder_ty),
                    ),
                })
            }
            ARRAY_BUILDER_BUILD_ARRAY_FQN
            | ARRAY_BUILDER_BUILD_ARRAY_STRING_FQN
            | "scoop.core.freeze" => {
                let element_ty = self.array_element_ty_from_array_ty(result_ty);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderBuildArray,
                    array_ty: result_ty,
                    element_ty,
                    mutable: false,
                    element: self
                        .value_transport_with_kind(element_ty, MirTransportKind::ArrayElement),
                })
            }
            ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN => {
                let element_ty = self.array_element_ty_from_array_ty(result_ty);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::BuilderBuildMutableArray,
                    array_ty: result_ty,
                    element_ty,
                    mutable: true,
                    element: self
                        .value_transport_with_kind(element_ty, MirTransportKind::ArrayElement),
                })
            }
            _ if callee_fqn.ends_with(".get") => Some(ArrayElementTransportMetadata {
                operation: ArrayTransportOperation::Get,
                array_ty: args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any),
                element_ty: result_ty,
                mutable: false,
                element: self.value_transport_with_kind(result_ty, MirTransportKind::ArrayElement),
            }),
            _ if callee_fqn.ends_with(".set") => {
                let array_ty = args
                    .first()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                let element_ty = args
                    .last()
                    .map(|arg| self.operand_ty(&arg.value))
                    .unwrap_or(self.builtins.any);
                Some(ArrayElementTransportMetadata {
                    operation: ArrayTransportOperation::Set,
                    array_ty,
                    element_ty,
                    mutable: true,
                    element: self.value_transport_with_boxing_reason(
                        element_ty,
                        MirTransportKind::ArrayElement,
                        MirBoxingReason::ArrayElement,
                        Some(array_ty),
                    ),
                })
            }
            _ => None,
        }
    }

    pub(in crate::mir::lower) fn array_element_ty_from_array_ty(&self, array_ty: TypeId) -> TypeId {
        match self.types.kind(array_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if matches!(
                    nominal.fqn.as_str(),
                    "scoop.core.Array"
                        | "scoop.core.MutableArray"
                        | "scoop.core.List"
                        | "scoop.core.MutableList"
                ) =>
            {
                nominal.args.first().copied().unwrap_or(self.builtins.any)
            }
            _ => self.builtins.any,
        }
    }

    pub(in crate::mir::lower) fn canonicalize_perform_args(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lowered_args: Vec<CallArg>,
    ) -> Option<(Vec<PerformArg>, PerformMetadata)> {
        let uses_typed_contracts = self.facts.uses_typed_contracts();
        if let Some(mut metadata) = self
            .facts
            .perform_metadata(self.source_path.as_path(), span)
            .filter(|metadata| {
                if uses_typed_contracts {
                    metadata.arg_mapping.len() == lowered_args.len()
                } else {
                    metadata
                        .arg_mapping
                        .iter()
                        .all(|idx| *idx < lowered_args.len())
                }
            })
            .cloned()
        {
            let perform_args = if uses_typed_contracts {
                lowered_args
                    .iter()
                    .enumerate()
                    .map(|(param_idx, arg)| PerformArg {
                        span: arg.span,
                        source_arg_index: metadata.arg_mapping[param_idx],
                        name: arg.name.clone(),
                        value: arg.value.clone(),
                    })
                    .collect::<Vec<_>>()
            } else {
                metadata
                    .arg_mapping
                    .iter()
                    .copied()
                    .filter_map(|arg_idx| lowered_args.get(arg_idx).map(|arg| (arg_idx, arg)))
                    .map(|(source_arg_index, arg)| PerformArg {
                        span: arg.span,
                        source_arg_index,
                        name: arg.name.clone(),
                        value: arg.value.clone(),
                    })
                    .collect::<Vec<_>>()
            };
            metadata.payload_transport = perform_args
                .iter()
                .map(|arg| {
                    let ty = self.operand_ty(&arg.value);
                    self.value_transport_with_boxing_reason(
                        ty,
                        MirTransportKind::EffectPayload,
                        MirBoxingReason::EffectPayload,
                        metadata.payload_tuple_ty,
                    )
                })
                .collect();
            return Some((perform_args, metadata));
        }

        if self.facts.uses_typed_contracts() {
            return None;
        }

        let info = self.facts.fallback_perform_site_info(span);
        let arg_mapping = info
            .map(|site| site.arg_mapping.as_slice())
            .filter(|mapping| mapping.len() == lowered_args.len())
            .map(|mapping| mapping.to_vec())
            .unwrap_or_else(|| (0..lowered_args.len()).collect());

        let perform_args = lowered_args
            .iter()
            .enumerate()
            .map(|(param_idx, arg)| PerformArg {
                span: arg.span,
                source_arg_index: arg_mapping[param_idx],
                name: arg.name.clone(),
                value: arg.value.clone(),
            })
            .collect::<Vec<_>>();

        let payload_tuple_ty = info.and_then(|site| site.payload_tuple_ty).or_else(|| {
            (perform_args.len() > 1).then(|| {
                self.types.ty_tuple(
                    perform_args
                        .iter()
                        .map(|arg| self.operand_ty(&arg.value))
                        .collect(),
                )
            })
        });

        let payload_component_tys = perform_args
            .iter()
            .map(|arg| self.operand_ty(&arg.value))
            .collect();
        let payload_transport = perform_args
            .iter()
            .map(|arg| {
                let ty = self.operand_ty(&arg.value);
                self.value_transport_with_boxing_reason(
                    ty,
                    MirTransportKind::EffectPayload,
                    MirBoxingReason::EffectPayload,
                    payload_tuple_ty,
                )
            })
            .collect();

        Some((
            perform_args,
            PerformMetadata {
                effect_ty: self.builtins.any,
                op_type_args: Vec::new(),
                result_ty,
                payload_tuple_ty,
                payload_component_tys,
                payload_transport,
                arg_mapping,
            },
        ))
    }

    pub(in crate::mir::lower) fn lower_call_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> LocalId {
        let result_ty = self.call_result_ty_from_callee(span, callee).unwrap_or(ty);
        let result = self.push_temp_local(span, result_ty);

        if let Some(resume_info) = self
            .facts
            .resume_call_info(self.source_path.as_path(), span)
            .cloned()
        {
            self.lower_resume_call_expr(span, result, callee, args, &resume_info);
            return result;
        }

        if self.lower_typed_call_expr(span, result, callee, args) {
            return result;
        }

        if self.facts.fallback_resume_site_matches(span) {
            panic!(
                "typed continuation resume contract missing before MIR lowering at {} {span:?} (suspends_outward={})",
                self.source_path.display(),
                self.facts.fallback_resume_site_suspends_outward(span),
            );
        }

        if self
            .facts
            .dispatch_site_kind_for_call(self.source_path.as_path(), span, callee, args)
            .is_some()
        {
            panic!(
                "typed dispatch contract missing before MIR lowering at {} {span:?}: {:?}",
                self.source_path.display(),
                callee.kind,
            );
        }

        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind
            && matches!(
                self.types.kind(ty),
                TypeKind::Value(ValueTypeKind::Option(_) | ValueTypeKind::Nominal(_))
            )
        {
            let Some(args) = self.lower_call_args(args) else {
                return result;
            };
            let payload = self.aggregate_transport(
                ty,
                AggregateTransportKind::EnumPayload,
                args.iter()
                    .map(|arg| (arg.name.clone(), self.operand_ty(&arg.value)))
                    .collect::<Vec<_>>(),
            );
            self.assign(
                span,
                result,
                Rvalue::EnumVariant {
                    enum_ty: ty,
                    variant_name: name.clone(),
                    args,
                    payload,
                },
            );
            return result;
        }

        panic!(
            "typed call-site contract missing before MIR lowering at {} {span:?}: {:?}",
            self.source_path.display(),
            callee.kind
        )
    }
}
