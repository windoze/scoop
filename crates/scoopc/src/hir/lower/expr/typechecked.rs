//! Typechecked-binding helpers, type-param substitution, ctor / struct-ctor / top-level-fun-value resolution.

#![allow(dead_code)]

use super::*;

impl<'a> HirLowering<'a> {
    pub(in crate::hir::lower) fn type_contains_param_for_direct_call_target(
        &self,
        ty: TypeId,
    ) -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match self.types.kind(id) {
                TypeKind::Param(_) => return true,
                TypeKind::StarProjection(star) => stack.push(star.read_ty),
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    stack.extend(nominal.args.iter().copied());
                    if let Some(eff) = &nominal.eff {
                        stack.extend(eff.terms.iter().copied());
                    }
                }
                TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
                TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                    stack.extend(elements.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Function(fun)) => {
                    if let Some(receiver) = fun.receiver {
                        stack.push(receiver);
                    }
                    stack.extend(fun.params.iter().copied());
                    stack.push(fun.return_ty);
                    stack.extend(fun.effects.terms.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Union(union)) => {
                    stack.extend(union.variants.iter().copied());
                }
                TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
                | TypeKind::Value(ValueTypeKind::Unit)
                | TypeKind::Value(ValueTypeKind::Nothing)
                | TypeKind::Value(ValueTypeKind::Bool)
                | TypeKind::Value(ValueTypeKind::Char)
                | TypeKind::Value(ValueTypeKind::Float64)
                | TypeKind::Value(ValueTypeKind::Float32)
                | TypeKind::Value(ValueTypeKind::Int)
                | TypeKind::Value(ValueTypeKind::UInt)
                | TypeKind::Value(ValueTypeKind::IntN(_))
                | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
            }
        }
        false
    }

    pub(in crate::hir::lower) fn effect_row_contains_param_for_direct_call_target(
        &self,
        row: &EffectRow,
    ) -> bool {
        row.terms
            .iter()
            .copied()
            .any(|ty| self.type_contains_param_for_direct_call_target(ty))
    }

    pub(in crate::hir::lower) fn materialized_top_level_fun_call_target_fqn(
        &mut self,
        call_span: Span,
    ) -> Option<String> {
        let binding = self.typechecked_top_level_fun_call_binding(call_span)?;
        self.materialized_direct_call_target_fqn_for_binding(&binding)
    }

    pub(in crate::hir::lower) fn dispatch_call_site(
        &self,
        span: Span,
        receiver_ty: TypeId,
    ) -> crate::hir::DispatchCallSite {
        crate::hir::DispatchCallSite::new(self.source.path().to_path_buf(), span, receiver_ty)
    }

    pub(in crate::hir::lower) fn materialized_devirtualized_dispatch_target_fqn(
        &mut self,
        call_span: Span,
        impl_member_fqn: &str,
    ) -> String {
        let Some(binding) = self.typechecked_top_level_fun_call_binding(call_span) else {
            return impl_member_fqn.to_string();
        };
        if binding.type_args.is_empty() && binding.eff_args.is_empty() {
            return impl_member_fqn.to_string();
        }
        let Some(overload) = self.fun_overload_by_fqn(impl_member_fqn) else {
            return impl_member_fqn.to_string();
        };
        self.materialized_instance_fqn_for_decl(
            impl_member_fqn,
            overload.symbol.decl_file.as_path(),
            overload.symbol.span,
            &binding.type_args,
            &binding.eff_args,
        )
    }

    pub(in crate::hir::lower) fn materialized_value_property_getter_target_fqn(
        &self,
        getter_fqn: &str,
        receiver_ty: TypeId,
    ) -> Option<String> {
        if !self.materialize_direct_call_targets {
            return None;
        }

        let (owner_fqn, _) = getter_fqn.rsplit_once('.')?;
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        if nominal.fqn != owner_fqn || nominal.args.is_empty() {
            return None;
        }
        if nominal
            .args
            .iter()
            .copied()
            .any(|ty| self.type_contains_param_for_direct_call_target(ty))
        {
            return None;
        }

        let (template, symbol_suffix) =
            self.generic_template_symbol_suffixes
                .iter()
                .find_map(|(template, suffix)| {
                    (template.fqn == getter_fqn).then_some((template, suffix.as_str()))
                })?;

        Some(stable_instance_fqn(
            self.types,
            template,
            &nominal.args,
            &[],
            symbol_suffix,
        ))
    }

    pub(in crate::hir::lower) fn top_level_callee_expr_with_fqn(
        &mut self,
        span: Span,
        fqn: String,
    ) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        }
    }

    pub(in crate::hir::lower) fn materialized_direct_call_target_fqn_for_binding(
        &self,
        binding: &crate::ast::TopLevelFunCallBinding,
    ) -> Option<String> {
        if !self.materialize_direct_call_targets {
            return None;
        }
        if binding.is_intrinsic || (binding.type_args.is_empty() && binding.eff_args.is_empty()) {
            return None;
        }
        if binding
            .type_args
            .iter()
            .copied()
            .any(|ty| self.type_contains_param_for_direct_call_target(ty))
            || binding
                .eff_args
                .iter()
                .any(|row| self.effect_row_contains_param_for_direct_call_target(row))
        {
            return None;
        }

        Some(self.materialized_instance_fqn_for_decl(
            &binding.fqn,
            binding.decl_file.as_path(),
            binding.decl_span,
            &binding.type_args,
            &binding.eff_args,
        ))
    }

    pub(in crate::hir::lower) fn fun_overload_for_call_binding(
        &self,
        binding: &crate::ast::TopLevelFunCallBinding,
    ) -> Option<crate::resolve::FunOverload> {
        let syms = self.index.by_fqn.get(&binding.fqn)?;
        syms.fun
            .iter()
            .find(|overload| {
                overload.symbol.decl_file == binding.decl_file
                    && overload.symbol.span == binding.decl_span
            })
            .cloned()
    }

    pub(in crate::hir::lower) fn typechecked_direct_call_expr(
        &mut self,
        span: Span,
        binding: &crate::ast::TopLevelFunCallBinding,
        args: Vec<CallArg>,
        ty: TypeId,
    ) -> Expr {
        let target_fqn = self
            .materialized_direct_call_target_fqn_for_binding(binding)
            .unwrap_or_else(|| binding.fqn.clone());
        let callee = self.top_level_callee_expr_with_fqn(span, target_fqn);
        Expr {
            span,
            ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
        }
    }

    pub(in crate::hir::lower) fn lower_typechecked_operator_direct_call_expr(
        &mut self,
        span: Span,
        binding: crate::ast::TopLevelFunCallBinding,
        args: Vec<CallArg>,
    ) -> (ExprKind, TypeId) {
        let ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);
        let call = self.typechecked_direct_call_expr(span, &binding, args, ty);
        (call.kind, call.ty)
    }

    fn builtin_scalar_ty_for_operator_owner_fqn(&mut self, owner_fqn: &str) -> Option<TypeId> {
        let kind = match owner_fqn {
            "scoop.core.Bool" => ValueTypeKind::Bool,
            "scoop.core.Char" => ValueTypeKind::Char,
            "scoop.core.Float64" => ValueTypeKind::Float64,
            "scoop.core.Float32" => ValueTypeKind::Float32,
            "scoop.core.Int" => ValueTypeKind::Int,
            "scoop.core.UInt" => ValueTypeKind::UInt,
            "scoop.core.Int8" => ValueTypeKind::IntN(8),
            "scoop.core.Int16" => ValueTypeKind::IntN(16),
            "scoop.core.Int32" => ValueTypeKind::IntN(32),
            "scoop.core.Int64" => ValueTypeKind::IntN(64),
            "scoop.core.UInt8" => ValueTypeKind::UIntN(8),
            "scoop.core.UInt16" => ValueTypeKind::UIntN(16),
            "scoop.core.UInt32" => ValueTypeKind::UIntN(32),
            "scoop.core.UInt64" => ValueTypeKind::UIntN(64),
            _ => return None,
        };
        Some(self.types.intern(TypeKind::Value(kind)))
    }

    fn operator_receiver_expected_expr(
        &mut self,
        binding: &crate::ast::TopLevelFunCallBinding,
    ) -> ExpectedExpr {
        let Some((owner_fqn, _)) = binding.fqn.rsplit_once('.') else {
            return ExpectedExpr::default();
        };
        self.builtin_scalar_ty_for_operator_owner_fqn(owner_fqn)
            .map(|ty| self.expected_expr_for_param_ty(ty))
            .unwrap_or_default()
    }

    pub(in crate::hir::lower) fn try_lower_typechecked_operator_overload_unary_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        operand: &ast::Expr,
    ) -> Option<(ExprKind, TypeId)> {
        let binding = self.typechecked_top_level_fun_call_binding(span)?;
        let expected = self.operator_receiver_expected_expr(&binding);
        let operand = self.lower_expr_with_expected(pkg_prefix, operand, expected);
        Some(self.lower_typechecked_operator_direct_call_expr(
            span,
            binding,
            vec![CallArg::Positional(operand)],
        ))
    }

    pub(in crate::hir::lower) fn try_lower_typechecked_operator_overload_binary_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<(ExprKind, TypeId)> {
        let binding = self.typechecked_top_level_fun_call_binding(span)?;
        let overload = self.fun_overload_for_call_binding(&binding);
        let lhs_expected = self.operator_receiver_expected_expr(&binding);
        let lhs = self.lower_expr_with_expected(pkg_prefix, lhs, lhs_expected);
        let rhs_expected = self.expected_expr_for_fun_call_arg(overload.as_ref(), rhs, 0);
        let rhs = self.lower_expr_with_expected(pkg_prefix, rhs, rhs_expected);
        Some(self.lower_typechecked_operator_direct_call_expr(
            span,
            binding,
            vec![CallArg::Positional(lhs), CallArg::Positional(rhs)],
        ))
    }

    pub(in crate::hir::lower) fn try_lower_typechecked_compare_to_binary_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op: ast::BinaryOp,
        op_span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<(ExprKind, TypeId)> {
        let binding = self.typechecked_top_level_fun_call_binding(span)?;
        let overload = self.fun_overload_for_call_binding(&binding);
        let lhs = self.lower_expr(pkg_prefix, lhs);
        let rhs_expected = self.expected_expr_for_fun_call_arg(overload.as_ref(), rhs, 0);
        let rhs = self.lower_expr_with_expected(pkg_prefix, rhs, rhs_expected);
        let compare_to_call = self.typechecked_direct_call_expr(
            span,
            &binding,
            vec![CallArg::Positional(lhs), CallArg::Positional(rhs)],
            self.builtins.int,
        );
        let zero = Expr {
            span,
            ty: self.builtins.int,
            kind: ExprKind::Literal(LiteralKind::SynthInt(0)),
        };
        Some((
            ExprKind::Binary {
                lhs: Box::new(compare_to_call),
                op,
                op_span,
                rhs: Box::new(zero),
            },
            self.builtins.bool_,
        ))
    }

    pub(in crate::hir::lower) fn apply_active_type_param_bindings(&mut self, ty: TypeId) -> TypeId {
        if self.type_param_scopes.is_empty() {
            return ty;
        }

        let mut bindings = std::collections::HashMap::new();
        for scope in &self.type_param_scopes {
            for (name, bound_ty) in scope {
                bindings.insert(name.clone(), *bound_ty);
            }
        }

        if bindings.is_empty() {
            ty
        } else {
            substitute_type_params(self.types, ty, &bindings)
        }
    }

    pub(in crate::hir::lower) fn apply_active_type_param_bindings_to_effect_row(
        &mut self,
        row: &crate::ty::EffectRow,
    ) -> crate::ty::EffectRow {
        let terms = row
            .terms
            .iter()
            .copied()
            .map(|term| self.apply_active_type_param_bindings(term))
            .collect();
        crate::ty::EffectRow::new(terms)
    }

    pub(in crate::hir::lower) fn typechecked_ctor_call_binding(
        &self,
        span: Span,
    ) -> Option<ast::CtorCallBinding> {
        self.file.typechecked_ctor_call_binding(span)
    }

    pub(in crate::hir::lower) fn struct_instance_from_type_id(
        &self,
        ty: TypeId,
    ) -> Option<(String, Vec<TypeId>)> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty).clone() else {
            return None;
        };
        if !matches!(
            self.type_kinds.get(&nominal.fqn),
            Some(ast::TypeKind::Struct)
        ) {
            return None;
        }
        Some((nominal.fqn, nominal.args))
    }

    pub(in crate::hir::lower) fn with_bound_struct_default_context<T>(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        type_params: &[String],
        concrete_args: &[TypeId],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.with_foreign_ast_context(decl_source, decl_file, |this| {
            this.push_type_param_bindings(
                type_params
                    .iter()
                    .cloned()
                    .zip(concrete_args.iter().copied()),
            );
            let result = f(this);
            this.pop_type_params();
            result
        })
    }

    pub(in crate::hir::lower) fn struct_default_param_expected_expr(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        type_params: &[String],
        concrete_args: &[TypeId],
        param: &DefaultArgParamInfo,
    ) -> ExpectedExpr {
        self.with_bound_struct_default_context(
            decl_source,
            decl_file,
            type_params,
            concrete_args,
            |this| {
                let value_ty = param
                    .ty_ref
                    .as_ref()
                    .map(|ty| this.lower_type_ref(ty))
                    .unwrap_or(this.builtins.any);
                ExpectedExpr {
                    value_ty: Some(value_ty),
                    array_lit_target: param
                        .ty_ref
                        .as_ref()
                        .and_then(|ty| this.array_lit_target_from_type_ref(ty)),
                    array_lit_ty: Some(value_ty),
                    struct_lit_ty: Some(value_ty),
                }
            },
        )
    }

    pub(in crate::hir::lower) fn struct_default_param_local_id(
        &mut self,
        decl_source: &'a crate::source::SourceFile,
        decl_file: &'a ast::File,
        decl_span: Span,
    ) -> crate::hir::SymbolId {
        self.with_foreign_ast_context(decl_source, decl_file, |this| {
            this.intern_local_symbol(decl_span, false)
        })
    }

    /// 无完整 typecheck 的 lowering/IR 测试入口仍可能需要识别简单 nominal ctor call。
    ///
    /// 说明：
    /// - 优先使用 typecheck side table；这里只作为 resolver 级 fallback；
    /// - 仅依据 resolver 的 ctor 候选集合与调用形状恢复“唯一可判定”的目标；
    /// - 若存在重载歧义、vararg/spread、或需要更深类型信息才能决定的情况，则返回 `None`，
    ///   让无 typecheck 路径保持保守失败，而不是猜错目标 ctor。
    pub(in crate::hir::lower) fn resolver_fallback_ctor_call_binding(
        &self,
        callee: &ast::ValueIdent,
        args: &[ast::Expr],
    ) -> Option<ast::CtorCallBinding> {
        let call = callee.call.as_ref()?;
        let mut ctor_types: Vec<String> = call
            .candidates
            .iter()
            .filter_map(|candidate| match candidate {
                ast::CallCandidate::Constructor { ty_fqn } => Some(ty_fqn.clone()),
                ast::CallCandidate::Fun { .. } => None,
            })
            .collect();
        ctor_types.sort();
        ctor_types.dedup();

        if ctor_types.len() != 1 {
            return None;
        }
        let owner_fqn = ctor_types.pop()?;

        let visible_ctors: Vec<&ConstructorOverload> = self
            .index
            .constructors
            .get(&owner_fqn)
            .into_iter()
            .flatten()
            .filter(|ctor| self.resolver_ctor_visible(ctor))
            .collect();

        if visible_ctors.is_empty() {
            return if args.is_empty() {
                Some(ast::CtorCallBinding {
                    owner_fqn,
                    ctor_span: None,
                    arg_mapping: Vec::new(),
                })
            } else {
                None
            };
        }

        let mut matched: Vec<(Option<Span>, Vec<Option<usize>>)> = visible_ctors
            .iter()
            .filter_map(|ctor| {
                self.resolver_fallback_ctor_arg_mapping(&ctor.params, args)
                    .map(|mapping| (Some(ctor.span), mapping))
            })
            .collect();

        if matched.len() != 1 {
            return None;
        }
        let (ctor_span, arg_mapping) = matched.pop()?;
        Some(ast::CtorCallBinding {
            owner_fqn,
            ctor_span,
            arg_mapping,
        })
    }

    pub(in crate::hir::lower) fn try_lower_struct_ctor_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
        typechecked_call_ty: Option<TypeId>,
    ) -> Option<(ExprKind, TypeId)> {
        // P4-T01h：识别 `Container<Int>(...)` 的 ctor 绑定时透明展开 `TypeApply` 外壳。
        let ast::ExprKind::Ident(id) = &self.transparent_call_callee(callee).kind else {
            return None;
        };
        let binding = self
            .typechecked_ctor_call_binding(call_span)
            .or_else(|| self.resolver_fallback_ctor_call_binding(id, args))?;
        if !matches!(
            self.type_kinds.get(&binding.owner_fqn),
            Some(ast::TypeKind::Struct)
        ) {
            return None;
        }

        let result_ty = typechecked_call_ty
            .unwrap_or_else(|| self.intern_nominal(binding.owner_fqn.clone(), Vec::new(), None));
        let (struct_fqn, concrete_args) = self
            .struct_instance_from_type_id(result_ty)
            .unwrap_or_else(|| (binding.owner_fqn.clone(), Vec::new()));

        let ctor = self
            .index
            .constructors
            .get(&binding.owner_fqn)?
            .iter()
            .find(|ctor| binding.ctor_span == Some(ctor.span))?;
        if binding.arg_mapping.len() != ctor.params.len() {
            return None;
        }

        let needs_defaults = binding.arg_mapping.iter().any(|slot| slot.is_none());
        if !needs_defaults {
            let mut fields = Vec::with_capacity(ctor.params.len());
            for (param_idx, param) in ctor.params.iter().enumerate() {
                let arg_idx = binding.arg_mapping.get(param_idx).copied().flatten()?;
                let arg = args.get(arg_idx)?;
                let value_expr = match &arg.kind {
                    ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                    _ => arg,
                };
                fields.push(StructLitField {
                    span: value_expr.span,
                    name: param.name.clone(),
                    name_span: call_span,
                    colon_span: call_span,
                    value: self.lower_expr(pkg_prefix, value_expr),
                });
            }
            return Some((
                ExprKind::StructLit {
                    ty: result_ty,
                    fields,
                },
                result_ty,
            ));
        }

        let info = self.default_arg_structs.get(&struct_fqn).cloned()?;
        if binding.arg_mapping.len() != info.params.len() {
            return None;
        }
        let (decl_source, decl_file) = self.decl_ast_context(&info.decl_file)?;
        let decl_pkg_prefix = package_prefix(decl_source, decl_file.package.as_ref());

        let expecteds: Vec<ExpectedExpr> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_expected_expr(
                    decl_source,
                    decl_file,
                    &info.type_params,
                    &concrete_args,
                    param,
                )
            })
            .collect();
        let param_ids: Vec<crate::hir::SymbolId> = info
            .params
            .iter()
            .map(|param| {
                self.struct_default_param_local_id(decl_source, decl_file, param.decl_span)
            })
            .collect();

        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in binding.arg_mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|slot| slot.is_none()) {
            return None;
        }

        let mut stmts: Vec<Stmt> = Vec::with_capacity(info.params.len() + 1);
        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param.get(arg_idx).copied().flatten()?;
            let param = info.params.get(param_idx)?;
            let expected = *expecteds.get(param_idx)?;
            let arg_value = match &arg.kind {
                ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                _ => arg,
            };
            let init = self.lower_expr_with_expected(pkg_prefix, arg_value, expected);
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(*param_ids.get(param_idx)?),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if binding
                .arg_mapping
                .get(param_idx)
                .copied()
                .flatten()
                .is_some()
            {
                continue;
            }
            let default_value = param.default_value.as_ref()?;
            let expected = *expecteds.get(param_idx)?;
            let init = self.with_bound_struct_default_context(
                decl_source,
                decl_file,
                &info.type_params,
                &concrete_args,
                |this| this.lower_expr_with_expected(&decl_pkg_prefix, default_value, expected),
            );
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(*param_ids.get(param_idx)?),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        let mut fields = Vec::with_capacity(info.params.len());
        for (param_idx, param) in info.params.iter().enumerate() {
            let expected = *expecteds.get(param_idx)?;
            fields.push(StructLitField {
                span: call_span,
                name: param.name.clone(),
                name_span: call_span,
                colon_span: call_span,
                value: Expr {
                    span: param.decl_span,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: *param_ids.get(param_idx)?,
                        name: param.name.clone(),
                        decl_span: param.decl_span,
                    }),
                },
            });
        }
        let struct_expr = Expr {
            span: call_span,
            ty: result_ty,
            kind: ExprKind::StructLit {
                ty: result_ty,
                fields,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: result_ty,
            kind: StmtKind::Expr(struct_expr),
        });

        Some((
            ExprKind::Block(Block {
                span: call_span,
                ty: result_ty,
                stmts,
            }),
            result_ty,
        ))
    }

    pub(in crate::hir::lower) fn resolver_ctor_visible(&self, ctor: &ConstructorOverload) -> bool {
        match ctor.visibility {
            Visibility::Public => true,
            Visibility::Internal => ctor.decl_cone == self.index.cone_of_source(self.source),
            Visibility::Private => ctor.decl_file == self.source.path(),
        }
    }

    pub(in crate::hir::lower) fn resolver_fallback_ctor_arg_mapping(
        &self,
        params: &[ParamSig],
        args: &[ast::Expr],
    ) -> Option<Vec<Option<usize>>> {
        if params.iter().any(|param| param.is_vararg) {
            return None;
        }

        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match &arg.kind {
                ast::ExprKind::NamedArg { .. } => {
                    seen_named = true;
                }
                ast::ExprKind::SpreadArg { .. } => {
                    return None;
                }
                _ => {
                    if seen_named {
                        return None;
                    }
                    positional_count = positional_count.saturating_add(1);
                }
            }
        }

        if positional_count > params.len() {
            return None;
        }

        let mut param_to_arg: Vec<Option<usize>> = vec![None; params.len()];
        for arg_idx in 0..positional_count {
            *param_to_arg.get_mut(arg_idx)? = Some(arg_idx);
        }

        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let ast::ExprKind::NamedArg { name, .. } = &arg.kind else {
                return None;
            };
            let name_text = name.text(self.source);
            let slot_idx = params.iter().position(|param| param.name == name_text)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(arg_idx);
        }

        for (idx, param) in params.iter().enumerate() {
            if param_to_arg.get(idx)?.is_some() {
                continue;
            }
            if !param.has_default {
                return None;
            }
        }

        Some(param_to_arg)
    }

    pub(in crate::hir::lower) fn synthetic_top_level_fun_value_param_span(
        &self,
        base_span: Span,
        ordinal: usize,
    ) -> Span {
        let offset = base_span.end.saturating_add(ordinal).saturating_add(1);
        Span::new(offset, offset)
    }

    pub(in crate::hir::lower) fn mangled_top_level_fun_value_fqn(
        &self,
        fqn: &str,
        decl_file: Option<&std::path::Path>,
        decl_span: Option<Span>,
        type_args: &[TypeId],
        eff_args: &[crate::ty::EffectRow],
    ) -> String {
        let type_args_concrete = type_args
            .iter()
            .all(|ty| !matches!(self.types.kind(*ty), TypeKind::Param(_)));
        let eff_args_concrete = eff_args.iter().all(|row| {
            row.terms
                .iter()
                .all(|ty| !matches!(self.types.kind(*ty), TypeKind::Param(_)))
        });
        if (type_args.is_empty() && eff_args.is_empty())
            || !type_args_concrete
            || !eff_args_concrete
        {
            return fqn.to_string();
        }
        match (decl_file, decl_span) {
            (Some(decl_file), Some(decl_span)) => self
                .materialized_instance_fqn_for_decl(fqn, decl_file, decl_span, type_args, eff_args),
            _ => {
                let mut args = type_args
                    .iter()
                    .map(|ty| self.types.display(*ty).to_string())
                    .collect::<Vec<_>>();
                args.extend(
                    eff_args
                        .iter()
                        .map(|row| format!("eff {}", self.format_effect_row_stable(row))),
                );
                format!("{fqn}::<{}>", args.join(", "))
            }
        }
    }

    pub(in crate::hir::lower) fn format_effect_row_stable(
        &self,
        row: &crate::ty::EffectRow,
    ) -> String {
        if row.terms.is_empty() {
            return "Pure".to_string();
        }
        row.terms
            .iter()
            .map(|ty| self.types.display(*ty).to_string())
            .collect::<Vec<_>>()
            .join(" + ")
    }

    pub(in crate::hir::lower) fn fallback_top_level_fun_value_target(
        &mut self,
        expr: &ast::Expr,
        expected: ExpectedExpr,
    ) -> Option<(String, Vec<TypeId>, Vec<crate::ty::EffectRow>, TypeId)> {
        let expected_fun_ty = expected.value_ty.filter(|ty| {
            matches!(
                self.types.kind(*ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            )
        })?;

        match &expr.kind {
            ast::ExprKind::Ident(id) => {
                let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
                    return None;
                };
                Some((fqn.clone(), Vec::new(), Vec::new(), expected_fun_ty))
            }
            ast::ExprKind::TypeApply { callee, args } => {
                let ast::ExprKind::Ident(id) = &callee.kind else {
                    return None;
                };
                let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
                    return None;
                };
                let mut type_args = Vec::new();
                let mut eff_args = Vec::new();
                for arg in args {
                    match arg {
                        ast::TypeRef::EffectRowArg { row, .. } => {
                            eff_args.push(self.lower_effect_row_expr(Some(row)));
                        }
                        other => type_args.push(self.lower_type_ref(other)),
                    }
                }
                Some((fqn.clone(), type_args, eff_args, expected_fun_ty))
            }
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn try_lower_top_level_fun_value_expr(
        &mut self,
        expr: &ast::Expr,
        expected: ExpectedExpr,
    ) -> Option<(ExprKind, TypeId)> {
        let (base_fqn, decl_file, decl_span, type_args, eff_args, fun_ty_id) =
            if let Some(fun_ref) = self.typechecked_top_level_fun_value_ref(expr.span) {
                let fun_ty_id = self.typechecked_expr_ty(expr.span).or(expected.value_ty)?;
                (
                    fun_ref.fqn,
                    Some(fun_ref.decl_file),
                    Some(fun_ref.decl_span),
                    fun_ref.type_args,
                    fun_ref.eff_args,
                    fun_ty_id,
                )
            } else {
                let (base_fqn, type_args, eff_args, fun_ty_id) =
                    self.fallback_top_level_fun_value_target(expr, expected)?;
                (base_fqn, None, None, type_args, eff_args, fun_ty_id)
            };
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(fun_ty_id).clone()
        else {
            return None;
        };

        let mut params: Vec<Param> =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        let mut call_args: Vec<CallArg> =
            Vec::with_capacity(fun_ty.params.len() + usize::from(fun_ty.receiver.is_some()));
        let mut ordinal = 0usize;

        if let Some(receiver_ty) = fun_ty.receiver {
            let decl_span = self.synthetic_top_level_fun_value_param_span(expr.span, ordinal);
            let id = self.intern_local_symbol(decl_span, false);
            let name = "receiver".to_string();
            params.push(Param {
                span: decl_span,
                id,
                name: name.clone(),
                ty: receiver_ty,
            });
            call_args.push(CallArg::Positional(Expr {
                span: decl_span,
                ty: receiver_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            }));
            ordinal += 1;
        }

        for (idx, param_ty) in fun_ty.params.iter().copied().enumerate() {
            let decl_span = self.synthetic_top_level_fun_value_param_span(expr.span, ordinal);
            let id = self.intern_local_symbol(decl_span, false);
            let name = format!("a{idx}");
            params.push(Param {
                span: decl_span,
                id,
                name: name.clone(),
                ty: param_ty,
            });
            call_args.push(CallArg::Positional(Expr {
                span: decl_span,
                ty: param_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            }));
            ordinal += 1;
        }

        let callee_fqn = self.mangled_top_level_fun_value_fqn(
            &base_fqn,
            decl_file.as_deref(),
            decl_span,
            &type_args,
            &eff_args,
        );
        let callee = Expr {
            span: expr.span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(callee_fqn.clone()),
                fqn: callee_fqn,
            }),
        };
        let body = Expr {
            span: expr.span,
            ty: fun_ty.return_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: call_args,
            },
        };

        Some((
            ExprKind::Closure(ClosureExpr {
                span: expr.span,
                id: self.alloc_closure_id(),
                at_safe_span: None,
                captures: Vec::new(),
                params,
                body: Box::new(body),
            }),
            fun_ty_id,
        ))
    }

    pub(in crate::hir::lower) fn array_lit_target_from_type_id(
        &self,
        ty: TypeId,
    ) -> Option<ArrayLitTarget> {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        match nominal.fqn.as_str() {
            "scoop.core.Array"
            | "scoop.core.List"
            | "scoop.collections.Set"
            | "scoop.collections.MapView" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableArray"
            | "scoop.core.MutableList"
            | "scoop.collections.MutableSet"
            | "scoop.collections.MutableMap" => Some(ArrayLitTarget::MutableArray),
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn array_lit_element_ty_from_type_id(
        &mut self,
        ty: TypeId,
    ) -> Option<TypeId> {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        self.array_lit_target_from_type_id(ty)?;
        nominal
            .args
            .first()
            .copied()
            .map(|arg| self.canonicalize_builtin_scalar_alias_ty(arg))
    }

    pub(in crate::hir::lower) fn array_lit_lowering_hint(
        &mut self,
        span: Span,
        expected: ExpectedExpr,
    ) -> Option<(ArrayLitTarget, TypeId, Option<TypeId>)> {
        let raw_result_ty = self
            .typechecked_expr_ty(span)
            .or(expected.array_lit_ty)
            .or(expected.struct_lit_ty)?;
        let result_ty = self.canonicalize_array_like_type_args(raw_result_ty);
        let target = self
            .array_lit_target_from_type_id(result_ty)
            .or(expected.array_lit_target)?;
        let element_ty = self.array_lit_element_ty_from_type_id(result_ty);
        Some((target, result_ty, element_ty))
    }

    pub(in crate::hir::lower) fn canonicalize_array_like_type_args(
        &mut self,
        ty: TypeId,
    ) -> TypeId {
        let TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) = self.types.kind(ty).clone()
        else {
            return ty;
        };
        if self.array_lit_target_from_type_id(ty).is_none() || nominal.args.len() != 1 {
            return ty;
        }

        let canonical_arg = self.canonicalize_builtin_scalar_alias_ty(nominal.args[0]);
        if canonical_arg == nominal.args[0] {
            return ty;
        }

        self.intern_nominal(nominal.fqn, vec![canonical_arg], nominal.eff)
    }

    pub(in crate::hir::lower) fn canonicalize_builtin_scalar_alias_ty(
        &mut self,
        ty: TypeId,
    ) -> TypeId {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty).clone() else {
            return ty;
        };
        if !nominal.args.is_empty() {
            return ty;
        }

        match nominal.fqn.as_str() {
            "scoop.core.Bool" => self.builtins.bool_,
            "scoop.core.Char" => self.builtins.char_,
            "scoop.core.Float64" => self.builtins.float64,
            "scoop.core.Float32" => self.builtins.float32,
            "scoop.core.Int" => self.builtins.int,
            "scoop.core.UInt" => self.builtins.uint,
            fqn => {
                if let Some(bits) = fqn
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u16>().ok())
                {
                    return self.types.ty_int_n(bits);
                }
                if let Some(bits) = fqn
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u16>().ok())
                {
                    return self.types.ty_uint_n(bits);
                }
                ty
            }
        }
    }

    pub(in crate::hir::lower) fn infer_array_lit_ty_from_lowered_elements(
        &mut self,
        elements: &[Expr],
    ) -> Option<TypeId> {
        let first_ty = elements.first()?.ty;
        if first_ty == self.builtins.any {
            return None;
        }
        if elements
            .iter()
            .skip(1)
            .any(|element| element.ty == self.builtins.any || element.ty != first_ty)
        {
            return None;
        }
        Some(self.intern_nominal("scoop.core.Array".to_string(), vec![first_ty], None))
    }

    /// 根据 FQN 获取函数签名（用于从函数参数类型向下传播 expected-type hint）。
    pub(in crate::hir::lower) fn fun_overload_by_fqn(
        &self,
        fqn: &str,
    ) -> Option<crate::resolve::FunOverload> {
        let syms = self.index.by_fqn.get(fqn)?;
        let overload = syms.fun.first()?;
        Some(overload.clone())
    }

    pub(in crate::hir::lower) fn fun_overload_return_ty(
        &mut self,
        overload: Option<&crate::resolve::FunOverload>,
    ) -> Option<TypeId> {
        let overload = overload?;
        overload
            .sig
            .return_ty
            .as_ref()
            .and_then(|ty| self.type_ref_ty_in_decl_context(&overload.symbol.decl_file, ty))
            .or(Some(self.builtins.unit))
    }

    /// 调用位置把 `TypeApply` 视为 callee 的透明外壳。
    ///
    /// 这样 `foo<T>()`、`obj.method<eff E>()` 与 `x.ext<U>()` 可以共用与非 `TypeApply`
    /// 相同的 lowering / 分派路径，而不会意外退回到值表达式处理。
    pub(in crate::hir::lower) fn transparent_call_callee<'b>(
        &self,
        callee: &'b ast::Expr,
    ) -> &'b ast::Expr {
        match &callee.kind {
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        }
    }

    pub(in crate::hir::lower) fn try_lower_retained_builtin_member_call_expr(
        &mut self,
        pkg_prefix: &str,
        _call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
        call_ty: TypeId,
    ) -> Option<(ExprKind, TypeId)> {
        let callee = self.transparent_call_callee(callee);
        let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
            return None;
        };
        if !self.should_keep_member_call_as_member_access(receiver, member) {
            return None;
        }

        // `String.length()` 继续保留 member-access callee 形状，但 call site 需要一个真实的
        // callable-typed callee，而不是把 `length` 误记成 `Int`。
        if self.source.slice(member.span) != "length" || !args.is_empty() {
            return None;
        }

        let receiver = self.lower_expr(pkg_prefix, receiver);
        let callee_ty = self
            .types
            .ty_function(None, Vec::new(), call_ty, EffectRow::pure(), true);
        let (kind, _) = self.lower_member_access_expr_from_receiver(
            pkg_prefix,
            callee.span,
            receiver,
            member,
            callee_ty,
        );
        let callee = Expr {
            span: callee.span,
            ty: callee_ty,
            kind,
        };
        Some((
            ExprKind::Call {
                callee: Box::new(callee),
                args: Vec::new(),
            },
            call_ty,
        ))
    }

    /// 尝试从 callee 表达式中提取“顶层函数 FQN”（用于向实参传播期望类型）。
    pub(in crate::hir::lower) fn callee_top_level_fqn<'b>(
        &self,
        callee: &'b ast::Expr,
    ) -> Option<&'b str> {
        // `callee<T>()`：在“调用 callee”位置仍把 `TypeApply` 视为透明包装；
        // 若其处于普通值表达式位置，则会提前经由 top-level function value side table 合成为 closure。
        let callee = self.transparent_call_callee(callee);
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        Some(fqn.as_str())
    }

    /// 为一次函数调用的某个实参计算 expected-type hint（目前仅用于数组字面量）。
    pub(in crate::hir::lower) fn expected_expr_for_fun_call_arg(
        &mut self,
        overload: Option<&crate::resolve::FunOverload>,
        arg: &ast::Expr,
        positional_index: usize,
    ) -> ExpectedExpr {
        // expected-type hint 当前既用于数组字面量 `[...]` 的 lowering（Array vs MutableArray），
        // 也用于把 `foo` / `foo<T>` 在值位置恢复成“顶层函数值 closure”形态。
        //
        // 注意：`FunSig` 的参数 `TypeRef` 可能来自**其它源文件**（sysroot/多文件编译单元），
        // 其 span 无法用当前文件的 `SourceFile` 回切；因此我们必须避免在“非数组字面量实参”
        // 的场景下无谓地解析参数类型，防止跨文件 span 误用导致 panic。
        let arg_is_array_lit = match &arg.kind {
            ast::ExprKind::ArrayLit { .. } => true,
            ast::ExprKind::NamedArg { value, .. } => {
                matches!(value.kind, ast::ExprKind::ArrayLit { .. })
            }
            _ => false,
        };
        let param_ty = match (overload, &arg.kind) {
            (Some(overload), ast::ExprKind::NamedArg { name, .. }) => {
                let name = name.text(self.source);
                overload
                    .sig
                    .params
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.ty.as_ref())
            }
            (Some(overload), _) => overload
                .sig
                .params
                .get(positional_index)
                .and_then(|p| p.ty.as_ref()),
            _ => None,
        };
        let decl_file = overload.map(|overload| overload.symbol.decl_file.as_path());
        let value_ty = match (decl_file, param_ty) {
            (Some(decl_file), Some(ty)) => self.type_ref_ty_in_decl_context(decl_file, ty),
            _ => None,
        };
        if !arg_is_array_lit {
            return ExpectedExpr {
                value_ty,
                array_lit_target: None,
                array_lit_ty: None,
                struct_lit_ty: value_ty,
            };
        }

        let array_lit_target = match (decl_file, param_ty) {
            (Some(decl_file), Some(ty)) => {
                self.array_lit_target_from_type_ref_in_decl_context(decl_file, ty)
            }
            _ => None,
        };
        let array_lit_ty = match (decl_file, param_ty) {
            (Some(decl_file), Some(ty)) => self.type_ref_ty_in_decl_context(decl_file, ty),
            _ => None,
        };

        ExpectedExpr {
            value_ty,
            array_lit_target,
            array_lit_ty,
            struct_lit_ty: value_ty,
        }
    }

    pub(in crate::hir::lower) fn call_arg_value_expr(arg: &ast::Expr) -> (&ast::Expr, bool) {
        let value = match &arg.kind {
            ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
            _ => arg,
        };
        match &value.kind {
            ast::ExprKind::SpreadArg { expr, .. } => (expr.as_ref(), true),
            _ => (value, false),
        }
    }

    pub(in crate::hir::lower) fn param_value_ty_from_plan(
        &mut self,
        plan: &CallableParamPlan,
        param: &DefaultArgParamInfo,
    ) -> TypeId {
        let Some(ty_ref) = param.ty_ref.as_ref() else {
            return self.builtins.any;
        };
        let Some((decl_source, decl_file)) = self.decl_ast_context(&plan.decl_file) else {
            return self.builtins.any;
        };
        self.with_foreign_ast_context(decl_source, decl_file, |this| {
            this.push_type_param_bindings(plan.type_param_bindings.clone());
            let ty = this.lower_type_ref(ty_ref);
            this.pop_type_params();
            ty
        })
    }

    pub(in crate::hir::lower) fn param_hir_ty_from_plan(
        &mut self,
        plan: &CallableParamPlan,
        param: &DefaultArgParamInfo,
    ) -> TypeId {
        let value_ty = self.param_value_ty_from_plan(plan, param);
        if param.is_vararg {
            self.intern_nominal("scoop.core.Array".to_string(), vec![value_ty], None)
        } else {
            value_ty
        }
    }

    pub(in crate::hir::lower) fn expected_expr_for_param_ty(&mut self, ty: TypeId) -> ExpectedExpr {
        ExpectedExpr {
            value_ty: Some(ty),
            array_lit_target: self.array_lit_target_from_type_id(ty),
            array_lit_ty: Some(ty),
            struct_lit_ty: Some(ty),
        }
    }

    pub(in crate::hir::lower) fn lower_default_arg_value(
        &mut self,
        plan: &CallableParamPlan,
        param: &DefaultArgParamInfo,
        expected: ExpectedExpr,
        overrides: &HashMap<Span, Span>,
    ) -> Option<Expr> {
        let default_value = param.default_value.as_ref()?;
        let (decl_source, decl_file) = self.decl_ast_context(&plan.decl_file)?;
        let decl_pkg_prefix = package_prefix(decl_source, decl_file.package.as_ref());
        Some(
            self.with_foreign_ast_context(decl_source, decl_file, |this| {
                this.with_local_decl_span_overrides(overrides.clone(), |this| {
                    this.push_type_param_bindings(plan.type_param_bindings.clone());
                    let lowered =
                        this.lower_expr_with_expected(&decl_pkg_prefix, default_value, expected);
                    this.pop_type_params();
                    lowered
                })
            }),
        )
    }

    pub(in crate::hir::lower) fn call_arg_binding_needs_block(
        &self,
        binding: &crate::ast::CallArgBinding,
    ) -> bool {
        let mut expected_arg_idx = 0usize;
        for param in &binding.params {
            match param {
                crate::ast::CallArgParamBinding::Receiver => {}
                crate::ast::CallArgParamBinding::Default => return true,
                crate::ast::CallArgParamBinding::Explicit(element) => {
                    if element.spread || element.arg_index != expected_arg_idx {
                        return true;
                    }
                    expected_arg_idx = expected_arg_idx.saturating_add(1);
                }
                crate::ast::CallArgParamBinding::Vararg(elements) => {
                    for element in elements {
                        if element.spread || element.arg_index != expected_arg_idx {
                            return true;
                        }
                        expected_arg_idx = expected_arg_idx.saturating_add(1);
                    }
                }
            }
        }
        false
    }

    pub(in crate::hir::lower) fn call_arg_binding_has_receiver(
        &self,
        binding: &crate::ast::CallArgBinding,
    ) -> bool {
        binding
            .params
            .iter()
            .any(|param| matches!(param, crate::ast::CallArgParamBinding::Receiver))
    }

    pub(in crate::hir::lower) fn lower_canonical_receiver_from_member_callee(
        &mut self,
        pkg_prefix: &str,
        callee: &ast::Expr,
    ) -> Option<Expr> {
        let callee = self.transparent_call_callee(callee);
        let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
            return None;
        };
        match self.resolved_member_for_lowering(member)? {
            ast::ResolvedMemberRef::Fun { fqn } => {
                let (owner_fqn, _) = fqn.rsplit_once('.')?;
                if let ast::ExprKind::Ident(id) = &receiver.kind
                    && id.resolved.is_none()
                    && self.source.slice(id.span) != "this"
                    && self.index.object_types.contains(owner_fqn)
                {
                    Some(self.synth_object_singleton_value_expr(owner_fqn, receiver.span))
                } else {
                    Some(self.lower_expr(pkg_prefix, receiver))
                }
            }
            ast::ResolvedMemberRef::ExtensionFun { .. } => {
                Some(self.lower_expr(pkg_prefix, receiver))
            }
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn plan_param_for_slot<'b>(
        &self,
        plan: Option<&'b CallableParamPlan>,
        param_index: usize,
        binding: &crate::ast::CallArgBinding,
    ) -> Option<(usize, &'b DefaultArgParamInfo)> {
        let plan = plan?;
        let receiver_before = binding
            .params
            .iter()
            .take(param_index)
            .filter(|param| matches!(param, crate::ast::CallArgParamBinding::Receiver))
            .count();
        let non_receiver_idx = param_index.checked_sub(receiver_before)?;
        plan.params
            .get(non_receiver_idx)
            .map(|param| (non_receiver_idx, param))
    }

    pub(in crate::hir::lower) fn lower_vararg_arg_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        source_args: &[ast::Expr],
        elements: &[crate::ast::CallArgElementBinding],
        elem_ty: TypeId,
        array_ty: TypeId,
    ) -> Option<Expr> {
        if elements.len() == 1
            && elements[0].spread
            && let Some(arg) = source_args.get(elements[0].arg_index)
        {
            let (value, _) = Self::call_arg_value_expr(arg);
            let expected = self.expected_expr_for_param_ty(array_ty);
            let lowered = self.lower_expr_with_expected(pkg_prefix, value, expected);
            if matches!(self.types.kind(lowered.ty), TypeKind::Ref(RefTypeKind::Nominal(n)) if n.fqn == "scoop.core.Array")
            {
                return Some(lowered);
            }
            if let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) =
                self.types.kind(lowered.ty).clone()
            {
                let temp_span = Span::new(call_span.start, call_span.start);
                let temp_id = self.intern_local_symbol(temp_span, false);
                let temp_name = "__spread_tuple".to_string();
                let tuple_decl = Stmt {
                    span: call_span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(ValDecl {
                        span: call_span,
                        id: Some(temp_id),
                        name: Some(temp_name.clone()),
                        mutable: false,
                        ty: lowered.ty,
                        init: Some(lowered),
                    }),
                };
                let mut array_elements = Vec::with_capacity(tuple_elems.len());
                for (idx, ty) in tuple_elems.iter().copied().enumerate() {
                    let receiver = Expr {
                        span: temp_span,
                        ty: self.types.ty_tuple(tuple_elems.clone()),
                        kind: ExprKind::VarRef(ValueRef::Local {
                            id: temp_id,
                            name: temp_name.clone(),
                            decl_span: temp_span,
                        }),
                    };
                    array_elements.push(Expr {
                        span: call_span,
                        ty,
                        kind: ExprKind::MemberAccess {
                            receiver: Box::new(receiver),
                            member: MemberAccess {
                                span: call_span,
                                name: format!("_{idx}"),
                                resolved: None,
                            },
                        },
                    });
                }
                let (array_kind, _) = self.build_array_lit_expr(
                    call_span,
                    array_elements,
                    ArrayLitTarget::Array,
                    array_ty,
                );
                let array_expr = Expr {
                    span: call_span,
                    ty: array_ty,
                    kind: array_kind,
                };
                return Some(Expr {
                    span: call_span,
                    ty: array_ty,
                    kind: ExprKind::Block(Block {
                        span: call_span,
                        ty: array_ty,
                        stmts: vec![
                            tuple_decl,
                            Stmt {
                                span: call_span,
                                ty: array_ty,
                                kind: StmtKind::Expr(array_expr),
                            },
                        ],
                    }),
                });
            }
            return Some(lowered);
        }

        let mut lowered_elements = Vec::new();
        for element in elements {
            let arg = source_args.get(element.arg_index)?;
            let (value, is_spread) = Self::call_arg_value_expr(arg);
            let expected =
                self.expected_expr_for_param_ty(if is_spread { array_ty } else { elem_ty });
            let lowered = self.lower_expr_with_expected(pkg_prefix, value, expected);
            if is_spread
                && let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) =
                    self.types.kind(lowered.ty).clone()
            {
                for (idx, ty) in tuple_elems.iter().copied().enumerate() {
                    lowered_elements.push(Expr {
                        span: call_span,
                        ty,
                        kind: ExprKind::MemberAccess {
                            receiver: Box::new(lowered.clone()),
                            member: MemberAccess {
                                span: call_span,
                                name: format!("_{idx}"),
                                resolved: None,
                            },
                        },
                    });
                }
                continue;
            }
            lowered_elements.push(lowered);
        }
        let (kind, _) =
            self.build_array_lit_expr(call_span, lowered_elements, ArrayLitTarget::Array, array_ty);
        Some(Expr {
            span: call_span,
            ty: array_ty,
            kind,
        })
    }
}
