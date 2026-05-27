//! Member access, ident, effect-op, elvis / safe-call / safe-member, handle, raise / runtime-error synthesis.

#![allow(dead_code)]

use super::*;

impl<'a> HirLowering<'a> {
    pub(in crate::hir::lower) fn lower_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let receiver = self.lower_expr(pkg_prefix, receiver);
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        self.lower_member_access_expr_from_receiver(pkg_prefix, span, receiver, member, result_ty)
    }

    pub(in crate::hir::lower) fn lower_splice_field_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        field: &ast::Expr,
    ) -> (ExprKind, TypeId) {
        let contract = self.typechecked_splice_field_contract(span);
        let Some(contract) = contract else {
            self.record_stage_error(
                span,
                "splice field access missing typed field contract",
                "HIR expression lowering",
            );
            return self.invalid_expr_kind_after_stage_error(span);
        };

        let receiver = Box::new(self.lower_expr(pkg_prefix, receiver));
        let ty = contract.field_ty;
        let member = MemberAccess {
            span: field.span,
            name: contract.field_name,
            resolved: Some(MemberRef::Value {
                id: self.symbols.intern_top_level(contract.field_fqn.clone()),
                fqn: contract.field_fqn,
            }),
        };

        (ExprKind::MemberAccess { receiver, member }, ty)
    }

    pub(in crate::hir::lower) fn nominal_fqn_for_ty(&self, ty: TypeId) -> Option<String> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Nominal(n)) | TypeKind::Ref(RefTypeKind::Nominal(n)) => {
                Some(n.fqn.clone())
            }
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn lower_member_access_expr_from_receiver(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: Expr,
        member: &ast::MemberIdent,
        result_ty: TypeId,
    ) -> (ExprKind, TypeId) {
        let resolved = self.resolved_member_for_lowering(member);

        // delegated property lowering（spec §10.4）：
        // `receiver.prop` → `receiver.prop$delegate.getValue(receiver, <PropertyMeta const>)`
        if let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref()
            && let Some(info) = self.delegated_properties.get(fqn).cloned()
        {
            match info {
                DelegatedPropertyInfo::Lazy(info) => {
                    return self.lower_lazy_delegated_property_get_from_receiver(
                        pkg_prefix,
                        member.span,
                        receiver,
                        &info,
                    );
                }
                DelegatedPropertyInfo::Generic(info) => {
                    let this_ref = receiver.clone();

                    let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                        member.span,
                        receiver,
                        &info,
                    );
                    let meta =
                        self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);

                    if let Some(class_fqn) = info.delegate_class_fqn.as_ref() {
                        let getter_fqn = format!("{class_fqn}.getValue");
                        let receiver_ty = delegate.ty;
                        let call = self.lower_synthetic_member_call_with_receiver_ty(
                            span,
                            delegate,
                            receiver_ty,
                            &getter_fqn,
                            vec![this_ref, meta],
                            result_ty,
                        );
                        return (call.kind, call.ty);
                    }

                    let callee = Expr {
                        span: member.span,
                        ty: self.builtins.any,
                        kind: ExprKind::MemberAccess {
                            receiver: Box::new(delegate),
                            member: MemberAccess {
                                span: member.span,
                                name: "getValue".to_string(),
                                resolved: None,
                            },
                        },
                    };

                    return (
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: vec![CallArg::Positional(this_ref), CallArg::Positional(meta)],
                        },
                        result_ty,
                    );
                }
                DelegatedPropertyInfo::Observable(info) => {
                    return self.lower_observable_vetoable_delegated_property_get_from_receiver(
                        member.span,
                        receiver,
                        fqn,
                        info.decl,
                        info.ty.as_ref(),
                        info.mutex_field_fqn,
                    );
                }
                DelegatedPropertyInfo::Vetoable(info) => {
                    return self.lower_observable_vetoable_delegated_property_get_from_receiver(
                        member.span,
                        receiver,
                        fqn,
                        info.decl,
                        info.ty.as_ref(),
                        info.mutex_field_fqn,
                    );
                }
                DelegatedPropertyInfo::MapBacked => {
                    // map-backed：值在初始化时被拷贝到真实字段，后续只读；
                    // 读取不需要额外同步，按普通字段访问处理。
                }
            }
        }

        // T0112：extension property access → desugar to getter call.
        // `receiver.extProp` → `extPropGetterFqn(receiver)`
        if let Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) = resolved.as_ref() {
            let callee_id = self.symbols.intern_top_level(fqn.clone());
            let callee = Expr {
                span: member.span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: callee_id,
                    fqn: fqn.clone(),
                }),
            };
            return (
                ExprKind::Call {
                    callee: Box::new(callee),
                    args: vec![CallArg::Positional(receiver)],
                },
                result_ty,
            );
        }

        // Computed property access → getter(receiver)。
        if let Some(ast::ResolvedMemberRef::Value { fqn }) = resolved.as_ref()
            && self.computed_property_getters.contains(fqn)
        {
            let getter_fqn = self
                .materialized_value_property_getter_target_fqn(fqn, receiver.ty)
                .unwrap_or_else(|| fqn.clone());
            let callee = Expr {
                span: member.span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: self.symbols.intern_top_level(getter_fqn.clone()),
                    fqn: getter_fqn,
                }),
            };
            return (
                ExprKind::Call {
                    callee: Box::new(callee),
                    args: vec![CallArg::Positional(receiver)],
                },
                result_ty,
            );
        }

        let receiver = Box::new(receiver);

        let resolved = resolved.as_ref().map(|r| self.lower_resolved_member_ref(r));

        let member = MemberAccess {
            span: member.span,
            name: self.source.slice(member.span).to_string(),
            resolved,
        };

        (ExprKind::MemberAccess { receiver, member }, result_ty)
    }

    pub(in crate::hir::lower) fn lower_resolved_member_ref(
        &mut self,
        resolved: &ast::ResolvedMemberRef,
    ) -> MemberRef {
        match resolved {
            ast::ResolvedMemberRef::Value { fqn } => MemberRef::Value {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::Fun { fqn } => MemberRef::Fun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionValue { fqn } => MemberRef::ExtensionValue {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionFun { fqn } => MemberRef::ExtensionFun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        }
    }

    pub(in crate::hir::lower) fn resolved_member_for_lowering(
        &self,
        member: &ast::MemberIdent,
    ) -> Option<ast::ResolvedMemberRef> {
        self.file
            .typechecked_member_resolved(member.span)
            .or_else(|| self.file.safe_member_access_resolved(member.span))
            .or_else(|| member.resolved.clone())
    }

    pub(in crate::hir::lower) fn should_keep_member_call_as_member_access(
        &mut self,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> bool {
        let Some(receiver_ty) = self.typechecked_expr_ty(receiver.span) else {
            return false;
        };
        let member_name = self.source.slice(member.span);

        if receiver_ty == self.builtins.string {
            return matches!(member_name, "byteLength" | "getByte");
        }

        false
    }

    pub(in crate::hir::lower) fn lower_ident_expr(
        &mut self,
        id: &ast::ValueIdent,
    ) -> (ExprKind, TypeId) {
        let text = self.source.slice(id.span);
        if text == "true" {
            return (
                ExprKind::Literal(LiteralKind::Bool(true)),
                self.builtins.bool_,
            );
        }
        if text == "false" {
            return (
                ExprKind::Literal(LiteralKind::Bool(false)),
                self.builtins.bool_,
            );
        }

        if text == "this"
            && let Some(decl_span) = self.lambda_this_decl_span
        {
            let ty = self
                .typechecked_binding_ty(decl_span)
                .or_else(|| self.typechecked_expr_ty(id.span))
                .or_else(|| self.synthetic_local_decl_ty(decl_span))
                .unwrap_or(self.builtins.any);
            return (
                ExprKind::VarRef(ValueRef::Local {
                    id: self.intern_local_symbol(decl_span, false),
                    name: "this".to_string(),
                    decl_span,
                }),
                ty,
            );
        }

        let Some(resolved) = id.resolved.as_ref() else {
            // 典型场景：enum variant ctor 的 callee（`Some(1)`）/0-参数 variant 值（`None`）；
            // resolver 会保留为“未 resolve”，让 typecheck 在期望类型语境下决议。
            return (
                ExprKind::UnresolvedIdent {
                    name: text.to_string(),
                },
                self.builtins.any,
            );
        };

        let resolved = match resolved {
            ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
                id: self.intern_local_symbol(*decl_span, false),
                name: name.clone(),
                decl_span: self.remap_local_decl_span(*decl_span),
            },
            ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        };

        let ty = match &resolved {
            ValueRef::Local { decl_span, .. } => self
                .typechecked_expr_ty(id.span)
                .or_else(|| self.typechecked_binding_ty(*decl_span))
                .or_else(|| self.synthetic_local_decl_ty(*decl_span))
                .unwrap_or(self.builtins.any),
            ValueRef::TopLevel { .. } => self
                .typechecked_expr_ty(id.span)
                .unwrap_or(self.builtins.any),
        };

        (ExprKind::VarRef(resolved), ty)
    }

    pub(in crate::hir::lower) fn synth_object_singleton_value_expr(
        &mut self,
        fqn: &str,
        span: Span,
    ) -> Expr {
        Expr {
            span,
            ty: self.intern_nominal(fqn.to_string(), Vec::new(), None),
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.to_string()),
                fqn: fqn.to_string(),
            }),
        }
    }

    pub(in crate::hir::lower) fn try_lower_effect_op_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        // `Effect.op<T>(...)`：HIR lowering 也把 TypeApply 视为“只包住 callee 的透明外壳”，
        // 以便 generic effect-op call 与普通 effect-op call 进入同一条 effect lowering 主线。
        let callee = self.transparent_call_callee(callee);

        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            return None;
        };
        let resolved = self.resolved_member_for_lowering(member);
        let Some(ast::ResolvedMemberRef::Fun { fqn }) = resolved.as_ref() else {
            return None;
        };
        if !self.is_effect_op_fqn(fqn) {
            return None;
        }

        let op = EffectOpRef {
            span: member.span,
            fqn: fqn.clone(),
            type_args: self
                .typechecked_effect_op_call_binding(call_span)
                .map(|binding| binding.op_type_args)
                .unwrap_or_default(),
        };
        let effect_ty = self
            .typechecked_performed_effect_ty(call_span)
            .unwrap_or(self.builtins.any);
        let arg_mapping = self
            .typechecked_effect_op_call_binding(call_span)
            .map(|binding| binding.arg_mapping)
            .unwrap_or_else(|| (0..args.len()).collect());
        let lowered_source_args: Vec<Expr> = args
            .iter()
            .map(|arg| {
                let (value, _) = Self::call_arg_value_expr(arg);
                self.lower_expr(pkg_prefix, value)
            })
            .collect();
        let args: Vec<CallArg> = arg_mapping
            .iter()
            .filter_map(|arg_idx| lowered_source_args.get(*arg_idx).cloned())
            .map(CallArg::Positional)
            .collect();
        let payload_tuple_ty = if args.len() > 1 {
            let elements = args.iter().map(Self::call_arg_value_ty).collect();
            Some(self.types.ty_tuple(elements))
        } else {
            None
        };
        self.effect_op_call_sites.insert(
            self.call_site(call_span),
            crate::hir::EffectOpCallInfo {
                arg_mapping,
                payload_tuple_ty,
            },
        );
        Some((
            ExprKind::Perform {
                effect_ty,
                op,
                args,
            },
            self.builtins.any,
        ))
    }

    pub(in crate::hir::lower) fn is_effect_op_fqn(&self, fqn: &str) -> bool {
        let Some(syms) = self.index.by_fqn.get(fqn) else {
            return false;
        };
        syms.fun
            .iter()
            .any(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
    }

    // ── T0108: nullable operators (`?.` and `!!`) desugar ──────────────────

    /// `expr!!` → `when (expr) { Some(v) -> v; None -> Raise.raise(RuntimeError.NullAssertionFailed) }`
    pub(in crate::hir::lower) fn lower_not_null_assert_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        expr: &ast::Expr,
        op_span: Span,
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, expr));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self.option_inner_ty(subject.ty).unwrap_or(result_ty);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__not_null_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: Expr {
                span: op_span,
                ty: result_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: v_sym,
                    name: "__not_null_v".to_string(),
                    decl_span: op_span,
                }),
            },
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_raise_null_assertion_failed(op_span),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// `lhs ?: rhs` → `when (lhs) { Some(v) -> v; None -> rhs }`
    ///
    /// 语义要求：
    /// - `lhs` 只求值一次；
    /// - `rhs` 仅在 `lhs` 为 `None` 时求值；
    /// - 结果类型与 typecheck 对 Elvis 的 inner type 推断保持一致。
    pub(in crate::hir::lower) fn lower_elvis_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        op_span: Span,
        rhs: &ast::Expr,
    ) -> Expr {
        let subject = Box::new(self.lower_expr(pkg_prefix, lhs));
        let rhs = self.lower_expr(pkg_prefix, rhs);
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(rhs.ty);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__elvis_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: Expr {
                span: op_span,
                ty: result_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: v_sym,
                    name: "__elvis_v".to_string(),
                    decl_span: op_span,
                }),
            },
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: rhs,
        };

        Expr {
            span,
            ty: result_ty,
            kind: ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
        }
    }

    /// `receiver?.field` → `when (receiver) { Some(v) -> Some(v.field); None -> None }`
    pub(in crate::hir::lower) fn lower_safe_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        op_span: Span,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, receiver));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let v_ref = Expr {
            span: op_span,
            ty: binder_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: v_sym,
                name: "__safe_v".to_string(),
                decl_span: op_span,
            }),
        };

        // T0152：Some 分支内与普通 member access 共享同一条 lowering 路径；
        // `?.` 只负责在外层包一层 `Some(...)` 并处理 `None` 分支。
        let inner_result_ty = self.option_inner_ty(result_ty).unwrap_or(self.builtins.any);
        let (inner_kind, inner_ty) = self.lower_member_access_expr_from_receiver(
            pkg_prefix,
            member.span,
            v_ref.clone(),
            member,
            inner_result_ty,
        );
        let inner_access = Expr {
            span: member.span,
            ty: inner_ty,
            kind: inner_kind,
        };

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__safe_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_some_wrap(op_span, result_ty, inner_access),
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_none(op_span, result_ty),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// `receiver?.method(args)` → `when (receiver) { Some(v) -> Some(v.method(args)); None -> None }`
    pub(in crate::hir::lower) fn lower_safe_call_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        op_span: Span,
        member: &ast::MemberIdent,
        args: &[ast::Expr],
    ) -> (ExprKind, TypeId) {
        let subject = Box::new(self.lower_expr(pkg_prefix, receiver));
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let binder_ty = self
            .option_inner_ty(subject.ty)
            .unwrap_or(self.builtins.any);
        let v_sym = self.intern_local_symbol(op_span, false);
        self.record_when_pat_binding_ty(op_span, binder_ty);

        let v_ref = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: v_sym,
                name: "__safe_v".to_string(),
                decl_span: op_span,
            }),
        };

        // Build the inner call `v.method(args)` using the same lowering strategies
        // as the normal Call path (extension fun → TopLevel, class member → TopLevel, fallback).
        let inner_call =
            self.lower_safe_call_inner_call(pkg_prefix, span, op_span, member, &v_ref, args);

        let some_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "Some".to_string(),
                args: vec![WhenPat::Bind {
                    span: op_span,
                    id: v_sym,
                    name: "__safe_v".to_string(),
                }],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_some_wrap(op_span, result_ty, inner_call),
        };

        let none_arm = WhenArm {
            span: op_span,
            pat: WhenPat::Variant {
                span: op_span,
                name_span: op_span,
                name: "None".to_string(),
                args: vec![],
            },
            guard: None,
            arrow_span: op_span,
            body: self.synth_none(op_span, result_ty),
        };

        (
            ExprKind::When {
                subject,
                arms: vec![some_arm, none_arm],
            },
            result_ty,
        )
    }

    /// Build the inner call for safe call desugaring.
    /// Mirrors the normal Call lowering: extension fun → TopLevel call, class member → TopLevel call.
    pub(in crate::hir::lower) fn lower_safe_call_inner_call(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op_span: Span,
        member: &ast::MemberIdent,
        v_ref: &Expr,
        args: &[ast::Expr],
    ) -> Expr {
        let lowered_args_without_receiver: Vec<CallArg> = args
            .iter()
            .map(|arg| self.lower_call_arg(pkg_prefix, arg))
            .collect();
        let resolved = self.resolved_member_for_lowering(member);

        // Extension function: `receiver?.ext(args)` → `ext(v, args...)`
        if let Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) = resolved.as_ref() {
            let mut all_args = Vec::with_capacity(lowered_args_without_receiver.len() + 1);
            all_args.push(CallArg::Positional(v_ref.clone()));
            all_args.extend(lowered_args_without_receiver);
            return Expr {
                span,
                ty: self.builtins.any,
                kind: ExprKind::Call {
                    callee: Box::new(Expr {
                        span: op_span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    }),
                    args: all_args,
                },
            };
        }

        // Ordinary member function: `receiver?.method(args)` → `Owner.method(v, args...)`
        if let Some(ast::ResolvedMemberRef::Fun { fqn }) = resolved.as_ref()
            && let Some((owner_fqn, _)) = fqn.as_str().rsplit_once('.')
        {
            let owner_is_struct =
                matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Struct));
            let owner_is_class =
                matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class));
            let owner_is_interface = matches!(
                self.type_kinds.get(owner_fqn),
                Some(ast::TypeKind::Interface)
            );
            let owner_is_object = self.index.object_types.contains(owner_fqn);
            if owner_is_struct || owner_is_class || owner_is_interface || owner_is_object {
                let explicit_args = lowered_args_without_receiver
                    .into_iter()
                    .map(|arg| match arg {
                        CallArg::Positional(expr) => expr,
                        CallArg::Named { value, .. } => value,
                    })
                    .collect();
                return self.lower_synthetic_member_call_with_receiver_ty(
                    span,
                    v_ref.clone(),
                    v_ref.ty,
                    fqn,
                    explicit_args,
                    self.builtins.any,
                );
            }
        }

        // Fallback: `v.method(args)` as MemberAccess call.
        let resolved = resolved.as_ref().map(|r| self.lower_resolved_member_ref(r));
        let member_name = self.source.slice(member.span).to_string();
        let callee = Expr {
            span: member.span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(v_ref.clone()),
                member: MemberAccess {
                    span: member.span,
                    name: member_name,
                    resolved,
                },
            },
        };
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: lowered_args_without_receiver,
            },
        }
    }

    // ── Synthesized HIR helpers for nullable desugar ───────────────────────

    /// Synthesize `Raise.raise(RuntimeError.NullAssertionFailed)` as a `Perform` node.
    pub(in crate::hir::lower) fn synth_raise_null_assertion_failed(&mut self, span: Span) -> Expr {
        let perform_span = Span::new(span.start, span.start);
        let error_expr = self.synth_runtime_error_unit_variant_expr(
            span,
            Self::RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN,
        );
        Expr {
            span: perform_span,
            ty: self.builtins.nothing,
            kind: ExprKind::Perform {
                effect_ty: self
                    .typechecked_performed_effect_ty(span)
                    .unwrap_or_else(|| self.synth_raise_runtime_error_effect_ty(span)),
                op: EffectOpRef {
                    span: perform_span,
                    fqn: Self::RAISE_RAISE_FQN.to_string(),
                    type_args: Vec::new(),
                },
                args: vec![CallArg::Positional(error_expr)],
            },
        }
    }

    pub(in crate::hir::lower) fn synth_runtime_error_unit_variant_expr(
        &mut self,
        span: Span,
        variant_fqn: &'static str,
    ) -> Expr {
        let (owner_fqn, variant_name) = variant_fqn
            .rsplit_once('.')
            .expect("runtime error variant helper requires qualified variant fqn");
        let runtime_error_ty = self.synth_runtime_error_ty(span);
        Expr {
            span,
            ty: runtime_error_ty,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(Expr {
                    span,
                    ty: runtime_error_ty,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(owner_fqn.to_string()),
                        fqn: owner_fqn.to_string(),
                    }),
                }),
                member: MemberAccess {
                    span,
                    name: variant_name.to_string(),
                    resolved: Some(MemberRef::Value {
                        id: self.symbols.intern_top_level(variant_fqn.to_string()),
                        fqn: variant_fqn.to_string(),
                    }),
                },
            },
        }
    }

    pub(in crate::hir::lower) fn synth_runtime_error_ty(&mut self, span: Span) -> TypeId {
        let runtime_error_path = ast::TypePath {
            span,
            segments: vec![
                ast::Ident::synthetic(span, "scoop"),
                ast::Ident::synthetic(span, "core"),
                ast::Ident::synthetic(span, "RuntimeError"),
            ],
            args: Vec::new(),
        };
        self.lower_type_path(&runtime_error_path)
    }

    pub(in crate::hir::lower) fn synth_raise_runtime_error_effect_ty(
        &mut self,
        span: Span,
    ) -> TypeId {
        let raise_path = ast::TypePath {
            span,
            segments: vec![
                ast::Ident::synthetic(span, "scoop"),
                ast::Ident::synthetic(span, "core"),
                ast::Ident::synthetic(span, "Raise"),
            ],
            args: vec![ast::TypeRef::Path(ast::TypePath {
                span,
                segments: vec![
                    ast::Ident::synthetic(span, "scoop"),
                    ast::Ident::synthetic(span, "core"),
                    ast::Ident::synthetic(span, "RuntimeError"),
                ],
                args: Vec::new(),
            })],
        };
        self.lower_type_path(&raise_path)
    }

    /// Synthesize `Some(inner)` and preserve the surrounding `Option<T>` result type.
    pub(in crate::hir::lower) fn synth_some_wrap(
        &self,
        span: Span,
        result_ty: TypeId,
        inner: Expr,
    ) -> Expr {
        Expr {
            span,
            ty: result_ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::UnresolvedIdent {
                        name: "Some".to_string(),
                    },
                }),
                args: vec![CallArg::Positional(inner)],
            },
        }
    }

    /// Synthesize `None()` and preserve the surrounding `Option<T>` result type.
    pub(in crate::hir::lower) fn synth_none(&self, span: Span, result_ty: TypeId) -> Expr {
        Expr {
            span,
            ty: result_ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span,
                    ty: self.builtins.any,
                    kind: ExprKind::UnresolvedIdent {
                        name: "None".to_string(),
                    },
                }),
                args: vec![],
            },
        }
    }

    pub(in crate::hir::lower) fn lower_handle_expr(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        arms: &[ast::HandleArm],
        finally: Option<&ast::Block>,
    ) -> HandleExpr {
        let body = self.lower_block(pkg_prefix, body);
        let arms = arms
            .iter()
            .map(|arm| self.lower_handle_arm(pkg_prefix, arm))
            .collect();
        let finally = finally.map(|b| self.lower_block(pkg_prefix, b));
        HandleExpr {
            body,
            arms,
            finally,
        }
    }

    pub(in crate::hir::lower) fn lower_handle_arm(
        &mut self,
        pkg_prefix: &str,
        arm: &ast::HandleArm,
    ) -> HandleArm {
        let kind = match arm.kind {
            ast::HandleArmKind::NonResuming => HandleArmKind::NonResuming,
            ast::HandleArmKind::EscapeContinuation { k_span } => {
                HandleArmKind::EscapeContinuation {
                    continuation: self.intern_local_symbol(k_span, false),
                }
            }
        };
        HandleArm {
            span: arm.span,
            op: self.lower_handle_op(pkg_prefix, &arm.op),
            kind,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    pub(in crate::hir::lower) fn lower_handle_op(
        &mut self,
        _pkg_prefix: &str,
        op: &ast::HandleOp,
    ) -> HandleOp {
        let effect_ty = self
            .typechecked_handle_arm_effect_ty(op.span)
            .unwrap_or_else(|| self.lower_type_path(&op.effect));
        let effect_fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(op.effect.clone()),
        );

        let op_name = op.op.text(self.source).to_string();
        let op_fqn = match effect_fqn {
            Some(effect_fqn) => format!("{effect_fqn}.{op_name}"),
            None => format!("{}.{}", self.source.slice(op.effect.span), op_name),
        };

        let binders = op
            .binders
            .iter()
            .map(|b| self.lower_handle_binder(b))
            .collect::<Vec<_>>();
        if binders.len() > 1 {
            let tuple_ty = self
                .types
                .ty_tuple(binders.iter().map(|binder| binder.ty).collect());
            self.handle_payload_tuple_tys
                .insert(self.call_site(op.span), tuple_ty);
        }

        HandleOp {
            span: op.span,
            effect_ty,
            op: EffectOpRef {
                span: op.op.span,
                fqn: op_fqn,
                type_args: self
                    .typechecked_handle_arm_op_type_args(op.span)
                    .unwrap_or_else(|| {
                        op.op_type_args
                            .iter()
                            .map(|arg| self.lower_type_ref(arg))
                            .collect()
                    }),
            },
            binders,
        }
    }

    pub(in crate::hir::lower) fn lower_handle_binder(
        &mut self,
        b: &ast::HandleBinder,
    ) -> HandleBinder {
        let ty =
            b.ty.as_ref()
                .map(|t| self.lower_type_ref(t))
                .or_else(|| self.typechecked_binding_ty(b.name.span))
                .unwrap_or(self.builtins.any);
        HandleBinder {
            span: b.span,
            id: self.intern_local_symbol(b.name.span, false),
            name: b.name.text(self.source).to_string(),
            ty,
        }
    }

    pub(in crate::hir::lower) fn lower_call_arg(
        &mut self,
        pkg_prefix: &str,
        arg: &ast::Expr,
    ) -> CallArg {
        let (value, _) = Self::call_arg_value_expr(arg);
        CallArg::Positional(self.lower_expr(pkg_prefix, value))
    }

    pub(in crate::hir::lower) fn call_arg_value_ty(arg: &CallArg) -> TypeId {
        match arg {
            CallArg::Positional(expr) => expr.ty,
            CallArg::Named { value, .. } => value.ty,
        }
    }

    /// 若该调用点满足“尾部默认参数可补齐”的规则，则把调用表达式 lowering 为一个 block：
    ///
    /// ```text
    /// f(a0, a1)   // 省略尾部默认参数
    /// =>
    /// {
    ///   val p0 = a0
    ///   val p1 = a1
    ///   val p2 = <default>
    ///   val p3 = <default>
    ///   f(p0, p1, p2, p3)
    /// }
    /// ```
    ///
    /// 说明：
    /// - 这样可以保证 default value 里对“更早参数”的引用能工作（通过局部 `val` 绑定）；
    /// - 也能保证“实参表达式”不会因简单替换而被重复求值（求值顺序与一次性语义可控）。
    pub(in crate::hir::lower) fn try_lower_default_args_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        let typechecked_call_ty = self.typechecked_expr_ty(call_span);
        let call_ty = typechecked_call_ty.unwrap_or(self.builtins.any);

        // 仅处理：顶层函数直接调用 `foo(...)`。
        // `callee<T>()`：HIR v0 视为透明包装（同 `lower_expr(TypeApply)`）。
        let callee = self.transparent_call_callee(callee);
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        let info = self.default_arg_funs.get(fqn).cloned()?;

        let provided = args.len();
        let total = info.params.len();
        if provided >= total {
            return None;
        }
        if provided < info.required {
            return None;
        }

        // Kotlin-like：命名实参之后不能再出现位置实参（与 typecheck 对齐；不支持 trailing-lambda 例外）。
        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match &arg.kind {
                ast::ExprKind::NamedArg { .. } => {
                    seen_named = true;
                }
                _ => {
                    if seen_named {
                        return None;
                    }
                    positional_count += 1;
                }
            }
        }
        if positional_count > total {
            return None;
        }

        // 将调用点的实参映射到形参槽位：
        // - 位置实参：按序绑定到 [0..positional_count)
        // - 命名实参：按 name 查找形参槽位
        let mut param_to_arg: Vec<Option<usize>> = vec![None; total];
        for arg_idx in 0..positional_count {
            *param_to_arg.get_mut(arg_idx)? = Some(arg_idx);
        }
        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let ast::ExprKind::NamedArg { name, .. } = &arg.kind else {
                return None;
            };
            let name_text = name.text(self.source).to_string();
            let slot_idx = info.params.iter().position(|p| p.name == name_text)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                // 同一形参不能被重复赋值（位置+命名/命名重复）。
                return None;
            }
            *slot = Some(arg_idx);
        }

        // 未填充的槽位必须有默认值。
        for (idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(idx)?.is_some() {
                continue;
            }
            param.default_value.as_ref()?;
        }

        // 反向映射：arg_idx -> param_idx（用于按调用点顺序求值实参）。
        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in param_to_arg.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|x| x.is_none()) {
            return None;
        }

        // 1) 先把“已提供的实参表达式”按参数名绑定为局部 val，避免重复求值。
        //    - 求值顺序：严格按调用点源码顺序（positional + named 的排列）。
        // 2) 再按形参顺序求值缺失的默认参数，并同样绑定为局部 val（供后续默认值引用）。
        let mut stmts: Vec<Stmt> = Vec::with_capacity(total + 1);

        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param.get(arg_idx).copied().flatten()?;
            let param = info.params.get(param_idx)?;
            let arg_value = match &arg.kind {
                ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                _ => arg,
            };
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let expected = ExpectedExpr {
                value_ty: Some(param_ty),
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
                array_lit_ty: Some(param_ty),
                struct_lit_ty: Some(param_ty),
            };
            let init = self.lower_expr_with_expected(pkg_prefix, arg_value, expected);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(param_idx)?.is_some() {
                continue;
            }
            let default_value = param.default_value.as_ref()?;
            let expected = ExpectedExpr {
                value_ty: param.ty_ref.as_ref().map(|t| self.lower_type_ref(t)),
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
                array_lit_ty: param.ty_ref.as_ref().map(|t| self.lower_type_ref(t)),
                struct_lit_ty: None,
            };
            let init = self.lower_expr_with_expected(pkg_prefix, default_value, expected);
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        // 最后一条语句：调用“完整参数形态”的原函数。
        let callee_id = self.symbols.intern_top_level(fqn.clone());
        let callee_expr = Expr {
            span: callee.span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: callee_id,
                fqn: fqn.clone(),
            }),
        };

        let mut full_args: Vec<CallArg> = Vec::with_capacity(total);
        for param in &info.params {
            let id = self.intern_local_symbol(param.decl_span, false);
            let vref = ValueRef::Local {
                id,
                name: param.name.clone(),
                decl_span: param.decl_span,
            };
            full_args.push(CallArg::Positional(Expr {
                span: param.decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(vref),
            }));
        }

        let call_expr = Expr {
            span: call_span,
            ty: call_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee_expr),
                args: full_args,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: call_expr.ty,
            kind: StmtKind::Expr(call_expr),
        });

        Some((
            ExprKind::Block(Block {
                span: call_span,
                ty: call_ty,
                stmts,
            }),
            call_ty,
        ))
    }

    pub(in crate::hir::lower) fn is_integer_type(&self, ty: TypeId) -> bool {
        if ty == self.builtins.int || ty == self.builtins.uint {
            return true;
        }

        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_))
        )
    }

    pub(in crate::hir::lower) fn is_char_type(&self, ty: TypeId) -> bool {
        ty == self.builtins.char_
    }

    /// 对齐 typecheck 阶段的最小规则：整数二元运算要求“相同的整数类型”，但允许一侧是整数字面量。
    ///
    /// 说明：HIR lowering 目前仅用于 dump/fixtures 与早期 codegen，因此这里的规则只覆盖：
    /// - 算术/位运算：`T op T -> T`（一侧为 numeric literal 时可吸收为另一侧的数值类型）
    /// - 移位：`T << Int -> T` / `T >> Int -> T`
    /// - 比较：`T < T -> Bool` 等
    /// - 相等：`T == T -> Bool` / `Bool == Bool -> Bool` / `Char == Char -> Bool`
    pub(in crate::hir::lower) fn lower_binary_expr_type(
        &self,
        lhs: &Expr,
        rhs: &Expr,
        op: ast::BinaryOp,
    ) -> TypeId {
        let unify_int_same_type = |lhs: &Expr, rhs: &Expr| -> Option<TypeId> {
            if lhs.ty == rhs.ty && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            let lhs_is_int_lit = matches!(lhs.kind, ExprKind::Literal(LiteralKind::Int));
            let rhs_is_int_lit = matches!(rhs.kind, ExprKind::Literal(LiteralKind::Int));

            if lhs_is_int_lit && self.is_integer_type(rhs.ty) {
                return Some(rhs.ty);
            }
            if rhs_is_int_lit && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            None
        };

        let unify_float_same_type = |lhs: &Expr, rhs: &Expr| -> Option<TypeId> {
            if lhs.ty == rhs.ty && self.is_float_type(lhs.ty) {
                return Some(lhs.ty);
            }

            let lhs_is_float_lit = matches!(lhs.kind, ExprKind::Literal(LiteralKind::Float64(_)));
            let rhs_is_float_lit = matches!(rhs.kind, ExprKind::Literal(LiteralKind::Float64(_)));

            if lhs_is_float_lit && self.is_float_type(rhs.ty) {
                return Some(rhs.ty);
            }
            if rhs_is_float_lit && self.is_float_type(lhs.ty) {
                return Some(lhs.ty);
            }

            None
        };

        match op {
            // arithmetic + bitwise: T op T -> T
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => unify_int_same_type(lhs, rhs)
                .or_else(|| unify_float_same_type(lhs, rhs))
                .or_else(|| {
                    (lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_)
                        .then_some(self.builtins.bool_)
                })
                .or_else(|| {
                    (lhs.ty == self.builtins.char_
                        && matches!(op, ast::BinaryOp::Add | ast::BinaryOp::Sub)
                        && (rhs.ty == self.builtins.int || rhs.ty == self.builtins.char_))
                        .then_some(match (op, rhs.ty) {
                            (ast::BinaryOp::Sub, ty) if ty == self.builtins.char_ => {
                                self.builtins.int
                            }
                            _ => self.builtins.char_,
                        })
                })
                .unwrap_or(self.builtins.any),

            // shifts: T << Int -> T
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if self.is_integer_type(lhs.ty) && rhs.ty == self.builtins.int {
                    lhs.ty
                } else {
                    self.builtins.any
                }
            }

            // comparisons: T < T -> Bool
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if unify_int_same_type(lhs, rhs).is_some()
                    || unify_float_same_type(lhs, rhs).is_some()
                    || (self.is_char_type(lhs.ty) && self.is_char_type(rhs.ty))
                {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // equality: (T == T) -> Bool; (Bool == Bool) -> Bool; (Char == Char) -> Bool
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    return self.builtins.bool_;
                }
                if self.is_char_type(lhs.ty) && self.is_char_type(rhs.ty) {
                    return self.builtins.bool_;
                }
                if unify_float_same_type(lhs, rhs).is_some() {
                    return self.builtins.bool_;
                }
                if unify_int_same_type(lhs, rhs).is_some() {
                    return self.builtins.bool_;
                }
                self.builtins.any
            }

            // boolean logic: Bool op Bool -> Bool
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // range/progression：正常路径会在 lowering 早期被展开为 `rangeTo(...)` 调用；
            // 这里保留 `Any` fallback，避免在无 typecheck 上下文的 dump-hir 路径里引入额外 interning 约束。
            ast::BinaryOp::RangeInclusive => self.builtins.any,

            // elvis not lowered in current HIR dump mode
            ast::BinaryOp::Elvis => self.builtins.any,
        }
    }
}
