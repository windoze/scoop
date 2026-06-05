//! FnLowering call lowering: arg canonicalization, direct/dispatch/intrinsic/funptr call variants.

#![allow(dead_code)]

use super::*;

impl<'a> FnLowering<'a> {
    fn selected_or_indexed_param_tys(
        &self,
        function: &FunctionTargetContract,
    ) -> Option<Vec<TypeId>> {
        if !function.param_tys().is_empty() {
            Some(function.param_tys().to_vec())
        } else {
            self.top_level_fun_param_tys.get(function.fqn()).cloned()
        }
    }

    fn direct_call_explicit_param_tys(
        &self,
        function: &FunctionTargetContract,
        explicit_arg_count: usize,
    ) -> Option<Vec<TypeId>> {
        let mut param_tys = self.selected_or_indexed_param_tys(function)?;
        if self.retained_gc_intrinsic_params_include_receiver(
            function.fqn(),
            explicit_arg_count,
            &param_tys,
        ) {
            param_tys.remove(0);
        }
        Some(param_tys)
    }

    fn retained_gc_intrinsic_params_include_receiver(
        &self,
        fqn: &str,
        explicit_arg_count: usize,
        param_tys: &[TypeId],
    ) -> bool {
        if !matches!(
            fqn,
            "scoop.core.GC.pin"
                | "scoop.core.GC.unpin"
                | "scoop.core.GC.handleNew"
                | "scoop.core.GC.handleGet"
                | "scoop.core.GC.handleDrop"
        ) || param_tys.len() != explicit_arg_count.saturating_add(1)
        {
            return false;
        }
        let Some((owner_fqn, _)) = fqn.rsplit_once('.') else {
            return false;
        };
        matches!(
            self.types.kind(param_tys[0]),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == owner_fqn
        )
    }

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
                self.lower_direct_call_expr(
                    span,
                    result,
                    function.fqn(),
                    args,
                    Some(&function),
                    None,
                );
                true
            }
            TypedCallSiteContract::MemberDirect(member) => {
                self.lower_direct_call_expr(
                    span,
                    result,
                    member.function().fqn(),
                    args,
                    Some(member.function()),
                    None,
                );
                true
            }
            TypedCallSiteContract::Extension { function, .. } => {
                self.lower_direct_call_expr(
                    span,
                    result,
                    function.fqn(),
                    args,
                    Some(&function),
                    None,
                );
                true
            }
            TypedCallSiteContract::Constructor(ctor) => {
                let result_ty = self.body.locals[result.as_u32() as usize].ty;
                let ctor_result_ty = self.constructor_result_ty_for_contract(&ctor);
                let ctor_result = if result_ty == ctor_result_ty {
                    result
                } else {
                    self.push_temp_local(span, ctor_result_ty)
                };
                self.lower_constructor_call_expr(span, ctor_result, &ctor, args);
                if ctor_result != result && !self.current_is_terminated() {
                    self.assign_use_to_local(span, result, Operand::Local(ctor_result));
                }
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
                self.lower_intrinsic_call_expr(span, result, &kind, &function, args)
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
        intrinsic_entry_name: Option<String>,
    ) {
        let arg_binding = function
            .and_then(FunctionTargetContract::arg_binding)
            .filter(|binding| !call_arg_binding_has_receiver(binding));
        let arg_binding = Self::active_hir_call_arg_binding(args, arg_binding);
        let expected_tys = function
            .and_then(|function| self.direct_call_explicit_param_tys(function, args.len()))
            .map(|param_tys| {
                self.source_arg_expected_tys_from_param_tys(
                    &param_tys,
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
        let intrinsic_entry_name = intrinsic_entry_name.or_else(|| {
            function
                .and_then(FunctionTargetContract::intrinsic_entry_name)
                .map(str::to_string)
        });
        let kind = CallKind::Direct {
            callee_fqn: callee_fqn.to_string(),
            stable_template_key: function
                .and_then(FunctionTargetContract::stable_template_key)
                .cloned()
                .map(Box::new),
            stable_instance_key: function
                .and_then(FunctionTargetContract::stable_instance_key)
                .cloned()
                .map(Box::new),
            intrinsic_entry_name,
            generic_type_args: function
                .map(FunctionTargetContract::type_args)
                .unwrap_or_default()
                .to_vec(),
            generic_eff_args: function
                .map(FunctionTargetContract::eff_args)
                .unwrap_or_default()
                .to_vec(),
        };
        let terminates_current_block = matches!(
            &kind,
            CallKind::Direct { callee_fqn, .. } if callee_fqn == "scoop.core.panic"
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
        ctor: &ConstructorCallTargetContract,
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
                    target_init_class_fqn: self
                        .types
                        .display(self.body.locals[result.as_u32() as usize].ty)
                        .to_string(),
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
        function: &FunctionTargetContract,
        args: &[hir::CallArg],
    ) -> bool {
        let callee_fqn = function.fqn();
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
                let site_id = self.fresh_site_id();
                self.assign(span, result, Rvalue::SizeOf { site_id, value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.alignOf")
                if name == "alignOf" =>
            {
                let value_ty = self.reflection_type_arg_for_call(span, "alignOf");
                let site_id = self.fresh_site_id();
                self.assign(span, result, Rvalue::AlignOf { site_id, value_ty });
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
                let site_id = self.fresh_site_id();
                self.assign(span, result, Rvalue::KindOf { site_id, value_ty });
                true
            }
            (TypedIntrinsicKind::Reflection { name }, "scoop.core.descOf") if name == "descOf" => {
                let value_ty = self.reflection_type_arg_for_call(span, "descOf");
                let site_id = self.fresh_site_id();
                self.assign(span, result, Rvalue::DescOf { site_id, value_ty });
                true
            }
            (TypedIntrinsicKind::Platform { name }, "scoop.core.getPlatform")
                if name == "getPlatform" =>
            {
                if !args.is_empty() {
                    panic!("typed getPlatform intrinsic must not publish arguments");
                }
                self.lower_platform_literal_expr(span, result);
                true
            }
            _ => {
                let entry_name = match kind {
                    TypedIntrinsicKind::NamedTable { entry_name, .. } => Some(entry_name.clone()),
                    _ => None,
                };
                self.lower_direct_call_expr(
                    span,
                    result,
                    callee_fqn,
                    args,
                    Some(function),
                    entry_name,
                );
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
        let selected_param_tys = self.selected_or_indexed_param_tys(member.function());
        let function_has_receiver = member
            .function()
            .arg_binding()
            .is_some_and(call_arg_binding_has_receiver)
            || selected_param_tys.as_ref().is_some_and(|param_tys| {
                self.dispatch_param_tys_include_receiver(
                    param_tys,
                    call_args.len(),
                    member.receiver_ty(),
                )
            });
        let expected_tys = selected_param_tys
            .map(|param_tys| {
                let explicit_param_tys = if function_has_receiver {
                    param_tys.get(1..).unwrap_or(&[])
                } else {
                    param_tys.as_slice()
                };
                self.source_arg_expected_tys_from_param_tys(
                    explicit_param_tys,
                    call_args.len(),
                    false,
                    arg_binding,
                )
            })
            .unwrap_or_else(|| vec![None; call_args.len()]);
        let Some(args) = self.lower_call_args_with_expected(call_args, &expected_tys) else {
            return;
        };
        let mut stable_candidate_keys = self
            .facts
            .dispatch_candidate_keys(self.source_path.as_path(), span)
            .to_vec();
        if let Some(stable_key) = member.function().stable_instance_key() {
            stable_candidate_keys.push(stable_key.clone());
        } else if let Some(stable_template_key) = member.function().stable_template_key()
            && let Ok(stable_key) = StableInstanceKey::from_type_arguments(
                stable_template_key.clone(),
                self.types,
                member.function().type_args(),
                member.function().eff_args(),
                &NoTypeParamResolver,
            )
        {
            stable_candidate_keys.push(stable_key);
        }
        stable_candidate_keys.sort_by_key(StableInstanceKey::canonical_text);
        stable_candidate_keys.dedup();
        let dispatch = DispatchMetadata {
            owner_fqn: member.owner_fqn().to_string(),
            member_name: member.member_name().to_string(),
            member_fqn: member.member_fqn().to_string(),
            member_decl_span: member.function().decl_span(),
            receiver_ty: member.receiver_ty(),
            stable_candidate_keys,
            stable_template_key: member
                .function()
                .stable_template_key()
                .cloned()
                .map(Box::new),
            generic_type_args: member.function().type_args().to_vec(),
            generic_eff_args: member.function().eff_args().to_vec(),
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

    fn dispatch_param_tys_include_receiver(
        &self,
        param_tys: &[TypeId],
        explicit_arg_count: usize,
        receiver_ty: TypeId,
    ) -> bool {
        if param_tys.len() != explicit_arg_count.saturating_add(1) {
            return false;
        }
        let Some(&first_param_ty) = param_tys.first() else {
            return false;
        };
        if first_param_ty == receiver_ty {
            return true;
        }
        self.nominal_fqn_for_ty(first_param_ty)
            .zip(self.nominal_fqn_for_ty(receiver_ty))
            .is_some_and(|(param_fqn, receiver_fqn)| param_fqn == receiver_fqn)
    }

    fn constructor_result_ty_for_contract(
        &mut self,
        ctor: &ConstructorCallTargetContract,
    ) -> TypeId {
        if self.nominal_fqn_for_ty(ctor.result_ty()).as_deref() == Some(ctor.owner_fqn()) {
            return ctor.result_ty();
        }
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: ctor.owner_fqn().to_string(),
                args: Vec::new(),
                eff: None,
            })))
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

    fn lower_platform_literal_expr(&mut self, span: Span, result: LocalId) {
        let result_ty = self.body.locals[result.as_u32() as usize].ty;
        if self.nominal_fqn_for_ty(result_ty).as_deref() != Some("scoop.core.Platform") {
            panic!("typed getPlatform intrinsic must return scoop.core.Platform");
        }

        let values = host_platform_literal_fields();
        let transport_fields = values
            .iter()
            .map(|(name, _)| (Some(name.clone()), self.builtins.string))
            .collect::<Vec<_>>();
        let fields = values
            .into_iter()
            .map(|(name, value)| crate::mir::StructLitField {
                span,
                name,
                value: Operand::Const(ConstValue::SynthString(value)),
            })
            .collect();
        self.assign(
            span,
            result,
            Rvalue::StructLit {
                fields,
                transport: self.aggregate_transport(
                    result_ty,
                    AggregateTransportKind::Struct,
                    transport_fields,
                ),
            },
        );
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
            Operand::Const(ConstValue::String | ConstValue::SynthString(_)) => self.builtins.string,
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
                    let kind = self.transport_kind_for_ty(capture.ty);
                    let transport = self.value_transport_with_boxing_reason(
                        capture.ty,
                        kind,
                        MirBoxingReason::ClosureCapture,
                        Some(env_ty),
                    );
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
            abi: self.call_abi_handoff(kind),
        }
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
                CallKind::Direct { callee_fqn, .. } => callee_fqn.as_str(),
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
        _kind: &CallKind,
    ) -> CallAbiHandoffMetadata {
        CallAbiHandoffMetadata::deferred_to_effect_facts()
    }

    pub(in crate::mir::lower) fn array_transport_metadata(
        &self,
        result_ty: TypeId,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Option<ArrayElementTransportMetadata> {
        let CallKind::Direct { callee_fqn, .. } = kind else {
            return None;
        };
        match intrinsic_base_fqn(callee_fqn.as_str()) {
            "scoop.core.push" => {
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
            "scoop.core.freeze" => {
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
        if let Some(mut metadata) = self
            .facts
            .perform_metadata(self.source_path.as_path(), span)
            .filter(|metadata| metadata.arg_mapping.len() == lowered_args.len())
            .cloned()
        {
            let perform_args = lowered_args
                .iter()
                .enumerate()
                .map(|(param_idx, arg)| PerformArg {
                    span: arg.span,
                    source_arg_index: metadata.arg_mapping[param_idx],
                    name: arg.name.clone(),
                    value: arg.value.clone(),
                })
                .collect::<Vec<_>>();
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

        let _ = result_ty;
        None
    }

    pub(in crate::mir::lower) fn lower_call_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> LocalId {
        let result_ty = if matches!(self.types.kind(ty), TypeKind::Ref(RefTypeKind::Any)) {
            self.call_result_ty_from_callee(span, callee).unwrap_or(ty)
        } else {
            ty
        };
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
                self.types.kind(result_ty),
                TypeKind::Value(ValueTypeKind::Option(_) | ValueTypeKind::Nominal(_))
            )
        {
            let Some(args) = self.lower_call_args(args) else {
                return result;
            };
            let payload = self.aggregate_transport(
                result_ty,
                AggregateTransportKind::EnumPayload,
                args.iter()
                    .map(|arg| (arg.name.clone(), self.operand_ty(&arg.value)))
                    .collect::<Vec<_>>(),
            );
            self.assign(
                span,
                result,
                Rvalue::EnumVariant {
                    enum_ty: result_ty,
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

fn host_platform_literal_fields() -> Vec<(String, String)> {
    let arch = host_llvm_target_arch();
    let vendor = host_llvm_target_vendor();
    let os = host_llvm_target_os();
    let env = host_llvm_target_env();
    let triple = if env.is_empty() {
        format!("{arch}-{vendor}-{os}")
    } else {
        format!("{arch}-{vendor}-{os}-{env}")
    };
    vec![
        ("triple".to_string(), triple),
        ("arch".to_string(), arch),
        ("vendor".to_string(), vendor),
        ("os".to_string(), os),
        ("env".to_string(), env),
    ]
}

fn host_llvm_target_arch() -> String {
    match std::env::consts::ARCH {
        "x86" => "i686".to_string(),
        arch => arch.to_string(),
    }
}

fn host_llvm_target_vendor() -> String {
    match std::env::consts::OS {
        "macos" | "ios" | "tvos" | "watchos" => "apple".to_string(),
        "windows" => "pc".to_string(),
        _ => "unknown".to_string(),
    }
}

fn host_llvm_target_os() -> String {
    match std::env::consts::OS {
        "macos" | "ios" | "tvos" | "watchos" => "darwin".to_string(),
        os => os.to_string(),
    }
}

fn host_llvm_target_env() -> String {
    if cfg!(target_env = "gnu") {
        "gnu".to_string()
    } else if cfg!(target_env = "musl") {
        "musl".to_string()
    } else if cfg!(target_env = "msvc") {
        "msvc".to_string()
    } else if cfg!(target_env = "sgx") {
        "sgx".to_string()
    } else {
        String::new()
    }
}
