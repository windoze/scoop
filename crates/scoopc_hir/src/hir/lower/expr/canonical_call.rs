//! Canonical call lowering, array literal, closure / lambda, struct literal, with-update expression.

#![allow(dead_code)]

use super::*;

impl<'a> HirLowering<'a> {
    pub(in crate::hir::lower) fn lower_canonical_call_expr(
        &mut self,
        request: CanonicalCallLoweringRequest<'_>,
    ) -> Option<(ExprKind, TypeId)> {
        let CanonicalCallLoweringRequest {
            pkg_prefix,
            call_span,
            callee,
            source_args,
            receiver,
            binding,
            plan,
            call_ty,
        } = request;
        if !self.call_arg_binding_needs_block(&binding) {
            let mut args = Vec::with_capacity(binding.params.len());
            for (param_idx, param_binding) in binding.params.iter().enumerate() {
                match param_binding {
                    crate::ast::CallArgParamBinding::Receiver => {
                        args.push(CallArg::Positional(receiver.clone()?));
                    }
                    crate::ast::CallArgParamBinding::Explicit(element) => {
                        let arg = source_args.get(element.arg_index)?;
                        let (value, _) = Self::call_arg_value_expr(arg);
                        let expected_ty = if let Some(plan_ref) = plan.as_ref() {
                            self.plan_param_for_slot(Some(plan_ref), param_idx, &binding)
                                .map(|(_, param)| self.param_hir_ty_from_plan(plan_ref, param))
                                .unwrap_or(self.builtins.any)
                        } else {
                            self.builtins.any
                        };
                        let expected = self.expected_expr_for_param_ty(expected_ty);
                        args.push(CallArg::Positional(
                            self.lower_expr_with_expected(pkg_prefix, value, expected),
                        ));
                    }
                    crate::ast::CallArgParamBinding::Vararg(elements) => {
                        let plan_ref = plan.as_ref()?;
                        let (_, param) =
                            self.plan_param_for_slot(Some(plan_ref), param_idx, &binding)?;
                        let elem_ty = self.param_value_ty_from_plan(plan_ref, param);
                        let array_ty = self.param_hir_ty_from_plan(plan_ref, param);
                        let expr = self.lower_vararg_arg_expr(
                            pkg_prefix,
                            call_span,
                            source_args,
                            elements,
                            elem_ty,
                            array_ty,
                        )?;
                        args.push(CallArg::Positional(expr));
                    }
                    crate::ast::CallArgParamBinding::Default => return None,
                }
            }
            return Some((
                ExprKind::Call {
                    callee: Box::new(callee),
                    args,
                },
                call_ty,
            ));
        }

        let plan = plan?;
        let mut stmts = Vec::new();
        let mut param_refs: Vec<Option<Expr>> = vec![None; binding.params.len()];
        let mut arg_refs: HashMap<usize, Expr> = HashMap::new();
        let mut overrides: HashMap<Span, Span> = HashMap::new();

        for (param_idx, param_binding) in binding.params.iter().enumerate() {
            if !matches!(param_binding, crate::ast::CallArgParamBinding::Receiver) {
                continue;
            }
            let receiver = receiver.clone()?;
            let (decl_span, id, name) =
                self.fresh_synthetic_local(call_span, "__call_receiver", false);
            let ty = receiver.ty;
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(id),
                    name: Some(name.clone()),
                    mutable: false,
                    ty,
                    init: Some(receiver),
                }),
            });
            param_refs[param_idx] = Some(Expr {
                span: decl_span,
                ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            });
        }

        for (arg_idx, source_arg) in source_args.iter().enumerate() {
            let mut expected_ty = None;
            let mut used = false;
            for (param_idx, param_binding) in binding.params.iter().enumerate() {
                match param_binding {
                    crate::ast::CallArgParamBinding::Explicit(element)
                        if element.arg_index == arg_idx =>
                    {
                        used = true;
                        if let Some((_, param)) =
                            self.plan_param_for_slot(Some(&plan), param_idx, &binding)
                        {
                            expected_ty = Some(self.param_hir_ty_from_plan(&plan, param));
                        }
                    }
                    crate::ast::CallArgParamBinding::Vararg(elements)
                        if elements.iter().any(|element| element.arg_index == arg_idx) =>
                    {
                        used = true;
                        if let Some((_, param)) =
                            self.plan_param_for_slot(Some(&plan), param_idx, &binding)
                        {
                            let (_, spread) = Self::call_arg_value_expr(source_arg);
                            expected_ty = Some(if spread {
                                self.param_hir_ty_from_plan(&plan, param)
                            } else {
                                self.param_value_ty_from_plan(&plan, param)
                            });
                        }
                    }
                    _ => {}
                }
            }
            if !used {
                continue;
            }
            let (value, _) = Self::call_arg_value_expr(source_arg);
            let expected =
                self.expected_expr_for_param_ty(expected_ty.unwrap_or(self.builtins.any));
            let init = self.lower_expr_with_expected(pkg_prefix, value, expected);
            let (decl_span, id, name) = self.fresh_synthetic_local(call_span, "__call_arg", false);
            let ty = init.ty;
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(id),
                    name: Some(name.clone()),
                    mutable: false,
                    ty,
                    init: Some(init),
                }),
            });
            arg_refs.insert(
                arg_idx,
                Expr {
                    span: decl_span,
                    ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id,
                        name,
                        decl_span,
                    }),
                },
            );
        }

        for (param_idx, param_binding) in binding.params.iter().enumerate() {
            if !matches!(param_binding, crate::ast::CallArgParamBinding::Default) {
                continue;
            }
            let (plan_idx, param) = self.plan_param_for_slot(Some(&plan), param_idx, &binding)?;
            let param_ty = self.param_hir_ty_from_plan(&plan, param);
            let expected = self.expected_expr_for_param_ty(param_ty);
            let init = self.lower_default_arg_value(&plan, param, expected, &overrides)?;
            let (decl_span, id, name) =
                self.fresh_synthetic_local(call_span, "__call_default", false);
            overrides.insert(param.decl_span, decl_span);
            let _ = plan_idx;
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(id),
                    name: Some(name.clone()),
                    mutable: false,
                    ty: param_ty,
                    init: Some(init),
                }),
            });
            param_refs[param_idx] = Some(Expr {
                span: decl_span,
                ty: param_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            });
        }

        for (param_idx, param_binding) in binding.params.iter().enumerate() {
            let crate::ast::CallArgParamBinding::Vararg(elements) = param_binding else {
                continue;
            };
            let (_, param) = self.plan_param_for_slot(Some(&plan), param_idx, &binding)?;
            let array_ty = self.param_hir_ty_from_plan(&plan, param);
            let expr = if elements.len() == 1 && elements[0].spread {
                let spread_ref = arg_refs.get(&elements[0].arg_index)?.clone();
                match self.types.kind(spread_ref.ty).clone() {
                    TypeKind::Ref(RefTypeKind::Nominal(n)) if n.fqn == "scoop.core.Array" => {
                        spread_ref
                    }
                    TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) => {
                        let mut array_elements = Vec::with_capacity(tuple_elems.len());
                        for (idx, ty) in tuple_elems.iter().copied().enumerate() {
                            array_elements.push(Expr {
                                span: call_span,
                                ty,
                                kind: ExprKind::MemberAccess {
                                    receiver: Box::new(spread_ref.clone()),
                                    member: MemberAccess {
                                        span: call_span,
                                        name: idx.to_string(),
                                        resolved: None,
                                    },
                                },
                            });
                        }
                        let (kind, _) = self.build_array_lit_expr(
                            call_span,
                            array_elements,
                            ArrayLitTarget::Array,
                            array_ty,
                        );
                        Expr {
                            span: call_span,
                            ty: array_ty,
                            kind,
                        }
                    }
                    _ => spread_ref,
                }
            } else {
                let mut array_elements = Vec::new();
                for element in elements {
                    let value = arg_refs.get(&element.arg_index)?.clone();
                    if element.spread {
                        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) =
                            self.types.kind(value.ty).clone()
                        else {
                            return None;
                        };
                        for (idx, ty) in tuple_elems.iter().copied().enumerate() {
                            array_elements.push(Expr {
                                span: call_span,
                                ty,
                                kind: ExprKind::MemberAccess {
                                    receiver: Box::new(value.clone()),
                                    member: MemberAccess {
                                        span: call_span,
                                        name: idx.to_string(),
                                        resolved: None,
                                    },
                                },
                            });
                        }
                    } else {
                        array_elements.push(value);
                    }
                }
                let (kind, _) = self.build_array_lit_expr(
                    call_span,
                    array_elements,
                    ArrayLitTarget::Array,
                    array_ty,
                );
                Expr {
                    span: call_span,
                    ty: array_ty,
                    kind,
                }
            };
            let (decl_span, id, name) =
                self.fresh_synthetic_local(call_span, "__call_vararg", false);
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span: call_span,
                    id: Some(id),
                    name: Some(name.clone()),
                    mutable: false,
                    ty: array_ty,
                    init: Some(expr),
                }),
            });
            param_refs[param_idx] = Some(Expr {
                span: decl_span,
                ty: array_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id,
                    name,
                    decl_span,
                }),
            });
        }

        for (param_idx, param_binding) in binding.params.iter().enumerate() {
            if let crate::ast::CallArgParamBinding::Explicit(element) = param_binding {
                param_refs[param_idx] = Some(arg_refs.get(&element.arg_index)?.clone());
            }
        }

        let args = param_refs
            .into_iter()
            .map(|expr| expr.map(CallArg::Positional))
            .collect::<Option<Vec<_>>>()?;
        let call_expr = Expr {
            span: call_span,
            ty: call_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: call_ty,
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

    /// 将 `[...]` 降到统一的 `MutableArray<T>` wrapper 调用形态。
    ///
    /// 形态（概念上）：
    /// ```text
    /// [e0, e1, e2]
    /// =>
    /// {
    ///   val __array_lit_tmp = mutableArrayNew<T>(capacity = 3)
    ///   __array_lit_tmp.push(e0)
    ///   __array_lit_tmp.push(e1)
    ///   __array_lit_tmp.push(e2)
    ///   __array_lit_tmp.freeze() // omitted for MutableArray<T> targets
    /// }
    /// ```
    pub(in crate::hir::lower) fn lower_array_lit_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        elements: &[ast::Expr],
        target: ArrayLitTarget,
        result_ty: TypeId,
        element_expected_ty: Option<TypeId>,
    ) -> (ExprKind, TypeId) {
        let lowered_elements: Vec<Expr> = elements
            .iter()
            .enumerate()
            .map(|(index, element)| {
                let expected = element_expected_ty
                    .map(|ty| ExpectedExpr {
                        value_ty: Some(ty),
                        array_lit_target: self.array_lit_target_from_type_id(ty),
                        array_lit_ty: Some(ty),
                        struct_lit_ty: Some(ty),
                    })
                    .unwrap_or_default();
                let lowered = self.lower_expr_with_expected(pkg_prefix, element, expected);
                match element_expected_ty {
                    Some(expected_ty)
                        if Self::array_lit_element_needs_expected_binding(element) =>
                    {
                        self.wrap_array_lit_element_with_expected_binding(
                            element.span,
                            index,
                            expected_ty,
                            lowered,
                        )
                    }
                    _ => lowered,
                }
            })
            .collect();
        self.build_array_lit_expr(span, lowered_elements, target, result_ty)
    }

    pub(in crate::hir::lower) fn array_lit_element_needs_expected_binding(
        element: &ast::Expr,
    ) -> bool {
        matches!(
            element.kind,
            ast::ExprKind::If { .. }
                | ast::ExprKind::When { .. }
                | ast::ExprKind::Block(_)
                | ast::ExprKind::DoBlock { .. }
                | ast::ExprKind::UnsafeBlock { .. }
                | ast::ExprKind::SafeBlock { .. }
                | ast::ExprKind::Handle { .. }
        ) || Self::array_lit_element_is_numeric_expected_binding_candidate(element)
    }

    pub(in crate::hir::lower) fn array_lit_element_is_numeric_expected_binding_candidate(
        element: &ast::Expr,
    ) -> bool {
        match &element.kind {
            ast::ExprKind::Unary { op, expr, .. } => {
                matches!(op, ast::UnaryOp::Neg | ast::UnaryOp::BitNot)
                    && Self::array_lit_element_is_numeric_literal_tree(expr)
            }
            ast::ExprKind::Binary { lhs, op, rhs, .. } => {
                matches!(
                    op,
                    ast::BinaryOp::Add
                        | ast::BinaryOp::Sub
                        | ast::BinaryOp::Mul
                        | ast::BinaryOp::Div
                        | ast::BinaryOp::Rem
                        | ast::BinaryOp::BitAnd
                        | ast::BinaryOp::BitXor
                        | ast::BinaryOp::BitOr
                        | ast::BinaryOp::Shl
                        | ast::BinaryOp::Shr
                ) && Self::array_lit_element_is_numeric_literal_tree(lhs)
                    && Self::array_lit_element_is_numeric_literal_tree(rhs)
            }
            _ => false,
        }
    }

    pub(in crate::hir::lower) fn array_lit_element_is_numeric_literal_tree(
        element: &ast::Expr,
    ) -> bool {
        match &element.kind {
            ast::ExprKind::IntLit | ast::ExprKind::FloatLit => true,
            ast::ExprKind::Unary { op, expr, .. } => {
                matches!(op, ast::UnaryOp::Neg | ast::UnaryOp::BitNot)
                    && Self::array_lit_element_is_numeric_literal_tree(expr)
            }
            ast::ExprKind::Binary { lhs, op, rhs, .. } => {
                matches!(
                    op,
                    ast::BinaryOp::Add
                        | ast::BinaryOp::Sub
                        | ast::BinaryOp::Mul
                        | ast::BinaryOp::Div
                        | ast::BinaryOp::Rem
                        | ast::BinaryOp::BitAnd
                        | ast::BinaryOp::BitXor
                        | ast::BinaryOp::BitOr
                        | ast::BinaryOp::Shl
                        | ast::BinaryOp::Shr
                ) && Self::array_lit_element_is_numeric_literal_tree(lhs)
                    && Self::array_lit_element_is_numeric_literal_tree(rhs)
            }
            _ => false,
        }
    }

    pub(in crate::hir::lower) fn wrap_array_lit_element_with_expected_binding(
        &mut self,
        span: Span,
        index: usize,
        expected_ty: TypeId,
        init: Expr,
    ) -> Expr {
        let decl_span = Span::new(span.start, span.start);
        let temp_id = self.intern_local_symbol(decl_span, false);
        let temp_name = format!("__array_elem_{index}");

        let val_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span,
                id: Some(temp_id),
                name: Some(temp_name.clone()),
                mutable: false,
                ty: expected_ty,
                init: Some(init),
            }),
        };

        let temp_ref = Expr {
            span,
            ty: expected_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: temp_id,
                name: temp_name,
                decl_span,
            }),
        };
        let temp_expr_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(temp_ref),
        };

        Expr {
            span,
            ty: expected_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: expected_ty,
                stmts: vec![val_stmt, temp_expr_stmt],
            }),
        }
    }

    pub(in crate::hir::lower) fn build_array_lit_expr(
        &mut self,
        span: Span,
        elements: Vec<Expr>,
        target: ArrayLitTarget,
        result_ty: TypeId,
    ) -> (ExprKind, TypeId) {
        let expr = self.lower_array_literal_via_mutable_array(
            elements,
            target,
            span,
            result_ty,
            "__array_lit_tmp",
        );
        (expr.kind, expr.ty)
    }

    pub(in crate::hir::lower) fn lower_array_literal_via_mutable_array(
        &mut self,
        elements: Vec<Expr>,
        target: ArrayLitTarget,
        span: Span,
        result_ty: TypeId,
        temp_prefix: &str,
    ) -> Expr {
        let element_ty = self
            .array_lit_element_ty_from_type_id(result_ty)
            .or_else(|| elements.first().map(|element| element.ty))
            .unwrap_or(self.builtins.any);
        let mutable_array_ty = self.intern_nominal(
            "scoop.core.MutableArray".to_string(),
            vec![element_ty],
            None,
        );
        let (array_decl_span, array_id, array_name) =
            self.fresh_synthetic_local(span, temp_prefix, false);

        let new_call_span = self.fresh_synthetic_call_site_span(span);
        let new_callee = self
            .top_level_callee_expr_with_fqn(new_call_span, Self::MUTABLE_ARRAY_NEW_FQN.to_string());
        let capacity_arg = Expr {
            span: new_call_span,
            ty: self.builtins.int,
            kind: ExprKind::Literal(LiteralKind::SynthInt(elements.len() as i64)),
        };
        let new_call = Expr {
            span: new_call_span,
            ty: mutable_array_ty,
            kind: ExprKind::Call {
                callee: Box::new(new_callee),
                args: vec![CallArg::Positional(capacity_arg)],
            },
        };

        let array_decl = ValDecl {
            span,
            id: Some(array_id),
            name: Some(array_name.clone()),
            mutable: false,
            ty: mutable_array_ty,
            init: Some(new_call),
        };

        let mut stmts: Vec<Stmt> = Vec::with_capacity(elements.len() + 2);
        stmts.push(Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(array_decl),
        });

        for element_expr in elements {
            let push_call_span = self.fresh_synthetic_call_site_span(element_expr.span);
            let array_ref = Expr {
                span: array_decl_span,
                ty: mutable_array_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: array_id,
                    name: array_name.clone(),
                    decl_span: array_decl_span,
                }),
            };

            let push_callee = self.top_level_callee_expr_with_fqn(
                push_call_span,
                Self::MUTABLE_ARRAY_PUSH_FQN.to_string(),
            );
            let push_call = Expr {
                span: push_call_span,
                ty: self.builtins.unit,
                kind: ExprKind::Call {
                    callee: Box::new(push_callee),
                    args: vec![
                        CallArg::Positional(array_ref),
                        CallArg::Positional(element_expr),
                    ],
                },
            };
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(push_call),
            });
        }

        let final_array_ref = Expr {
            span: array_decl_span,
            ty: mutable_array_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: array_id,
                name: array_name,
                decl_span: array_decl_span,
            }),
        };

        let result_expr = match target {
            ArrayLitTarget::Array => {
                let freeze_call_span = self.fresh_synthetic_call_site_span(span);
                let freeze_callee = self.top_level_callee_expr_with_fqn(
                    freeze_call_span,
                    Self::MUTABLE_ARRAY_FREEZE_FQN.to_string(),
                );
                Expr {
                    span: freeze_call_span,
                    ty: result_ty,
                    kind: ExprKind::Call {
                        callee: Box::new(freeze_callee),
                        args: vec![CallArg::Positional(final_array_ref)],
                    },
                }
            }
            ArrayLitTarget::MutableArray => Expr {
                ty: result_ty,
                ..final_array_ref
            },
        };
        stmts.push(Stmt {
            span,
            ty: result_expr.ty,
            kind: StmtKind::Expr(result_expr),
        });

        Expr {
            span,
            ty: result_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: result_ty,
                stmts,
            }),
        }
    }

    pub(in crate::hir::lower) fn lower_call_arg_with_expected(
        &mut self,
        pkg_prefix: &str,
        arg: &ast::Expr,
        expected: ExpectedExpr,
    ) -> CallArg {
        let (value, _) = Self::call_arg_value_expr(arg);
        CallArg::Positional(self.lower_expr_with_expected(pkg_prefix, value, expected))
    }

    pub(in crate::hir::lower) fn lower_call_arg_with_expected_preserving_name(
        &mut self,
        pkg_prefix: &str,
        arg: &ast::Expr,
        expected: ExpectedExpr,
    ) -> CallArg {
        let ast::ExprKind::NamedArg { name, value, .. } = &arg.kind else {
            return self.lower_call_arg_with_expected(pkg_prefix, arg, expected);
        };
        CallArg::Named {
            name: name.text(self.source).to_string(),
            name_span: name.span,
            value: self.lower_expr_with_expected(pkg_prefix, value, expected),
        }
    }

    /// T0113: Lower call arguments when the callee has a vararg parameter.
    ///
    /// Strategy:
    /// - Args before the vararg index are lowered as normal positional args.
    /// - Args at and after the vararg index (up to the end) are collected:
    ///   - If a single spread arg `*arr`: pass the inner expression directly as the array.
    ///   - Otherwise: wrap individual args into an array literal using the `MutableArray<T>` path.
    /// - The vararg slot becomes a single `CallArg::Positional(Array<T>)` expression.
    pub(in crate::hir::lower) fn lower_call_args_with_vararg(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        args: &[ast::Expr],
        overload: Option<&crate::resolve::FunOverload>,
        vararg_idx: usize,
    ) -> Vec<CallArg> {
        let mut out: Vec<CallArg> = Vec::with_capacity(args.len());
        let mut positional_index = 0usize;
        let mut vararg_args: Vec<&ast::Expr> = Vec::new();
        let mut has_spread = false;

        for arg in args {
            // Named args are passed through without affecting positional index.
            if let ast::ExprKind::NamedArg { .. } = &arg.kind {
                let expected = self.expected_expr_for_fun_call_arg(overload, arg, positional_index);
                out.push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                continue;
            }

            if positional_index < vararg_idx {
                // Pre-vararg: normal positional arg.
                let expected = self.expected_expr_for_fun_call_arg(overload, arg, positional_index);
                out.push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
            } else {
                // Vararg slot: collect for later wrapping.
                if matches!(&arg.kind, ast::ExprKind::SpreadArg { .. }) {
                    has_spread = true;
                }
                vararg_args.push(arg);
            }
            positional_index = positional_index.saturating_add(1);
        }

        // Build the vararg array arg.
        let vararg_expr = if vararg_args.is_empty() {
            // No args for vararg slot: pass an empty array.
            self.synth_empty_array_lit(call_span)
        } else if vararg_args.len() == 1 && has_spread {
            // Single spread arg: unwrap and pass the inner expression directly.
            let arg = vararg_args[0];
            match &arg.kind {
                ast::ExprKind::SpreadArg { expr: inner, .. } => self.lower_expr(pkg_prefix, inner),
                _ => unreachable!("has_spread is true but arg is not SpreadArg"),
            }
        } else {
            // Individual args: wrap in an array literal using the same path as array literals.
            let elements: Vec<&ast::Expr> = vararg_args
                .into_iter()
                .map(|arg| match &arg.kind {
                    // Unwrap spread args — this is a mixed case, currently unsupported;
                    // fall back to passing the inner expr as an element.
                    ast::ExprKind::SpreadArg { expr: inner, .. } => inner.as_ref(),
                    _ => arg,
                })
                .collect();
            self.synth_array_lit_from_exprs(pkg_prefix, call_span, &elements)
        };

        out.push(CallArg::Positional(vararg_expr));
        out
    }

    /// Synthesize an empty array literal expression.
    pub(in crate::hir::lower) fn synth_empty_array_lit(&mut self, span: Span) -> Expr {
        self.synth_array_lit_from_exprs("", span, &[])
    }

    /// Synthesize an array literal from a list of AST expressions.
    ///
    /// Uses the same `MutableArray<T>` path as `lower_array_lit_expr`.
    pub(in crate::hir::lower) fn synth_array_lit_from_exprs(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        elements: &[&ast::Expr],
    ) -> Expr {
        let lowered_elements: Vec<Expr> = elements
            .iter()
            .map(|element| self.lower_expr(pkg_prefix, element))
            .collect();
        let result_ty = self
            .infer_array_lit_ty_from_lowered_elements(&lowered_elements)
            .unwrap_or_else(|| {
                self.intern_nominal(
                    "scoop.core.Array".to_string(),
                    vec![self.builtins.any],
                    None,
                )
            });
        self.lower_array_literal_via_mutable_array(
            lowered_elements,
            ArrayLitTarget::Array,
            span,
            result_ty,
            "__vararg_array",
        )
    }

    pub(in crate::hir::lower) fn alloc_closure_id(&mut self) -> ClosureId {
        let id = ClosureId(self.next_closure);
        self.next_closure = self.next_closure.saturating_add(1);
        id
    }

    pub(in crate::hir::lower) fn lower_lambda_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lam: &ast::LambdaExpr,
    ) -> (ExprKind, TypeId) {
        let id = self.alloc_closure_id();
        let typechecked_fun_ty = self.typechecked_expr_ty(span).and_then(|ty| {
            let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(ty) else {
                return None;
            };
            Some((ty, fun_ty.clone()))
        });

        let receiver_this_decl_span = typechecked_fun_ty.as_ref().and_then(|(_, fun_ty)| {
            fun_ty
                .receiver
                .map(|_| ast::synthetic_lambda_receiver_this_decl_span(span))
        });
        let mut params: Vec<Param> = receiver_this_decl_span
            .and_then(|decl_span| {
                typechecked_fun_ty
                    .as_ref()
                    .and_then(|(_, fun_ty)| fun_ty.receiver.map(|ty| (decl_span, ty)))
            })
            .map(|(decl_span, ty)| Param {
                span: decl_span,
                id: self.intern_local_symbol(decl_span, false),
                name: "this".to_string(),
                ty,
            })
            .into_iter()
            .collect();
        if lam.params.is_empty()
            && lam.arrow_span.is_none()
            && receiver_this_decl_span.is_none()
            && let Some(param_ty) = typechecked_fun_ty
                .as_ref()
                .and_then(|(_, fun_ty)| (fun_ty.params.len() == 1).then(|| fun_ty.params[0]))
        {
            let decl_span = ast::synthetic_lambda_implicit_it_decl_span(span);
            params.push(Param {
                span: decl_span,
                id: self.intern_local_symbol(decl_span, false),
                name: "it".to_string(),
                ty: param_ty,
            });
        }
        params.extend(lam.params.iter().enumerate().map(|(idx, p)| {
            let name = p.name.text(self.source).to_string();
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .or_else(|| {
                        typechecked_fun_ty
                            .as_ref()
                            .and_then(|(_, fun_ty)| fun_ty.params.get(idx).copied())
                    })
                    .unwrap_or(self.builtins.any);
            Param {
                span: p.name.span,
                id: self.intern_local_symbol(p.name.span, false),
                name,
                ty,
            }
        }));
        let body = Box::new(match receiver_this_decl_span {
            Some(receiver_this_decl_span) => self
                .with_lambda_this_decl_span(Some(receiver_this_decl_span), |this| {
                    this.lower_expr(pkg_prefix, lam.body.as_ref())
                }),
            None => self.lower_expr(pkg_prefix, lam.body.as_ref()),
        });
        let captures = compute_closure_captures(&params, body.as_ref(), &self.local_mutability);
        (
            ExprKind::Closure(ClosureExpr {
                span,
                id,
                at_safe_span: lam.at_safe_span,
                captures,
                params,
                body,
            }),
            typechecked_fun_ty
                .map(|(ty, _)| ty)
                .unwrap_or(self.builtins.any),
        )
    }

    /// 把 AST 的 struct literal（`Type { field: expr, ... }`）降低为 HIR 表示。
    ///
    /// 说明：
    /// - 当前 lowering 不做字段存在性/类型匹配检查（这些属于 typecheck，参见 TODO T0423）；
    /// - 这里只保留“目标类型 + 字段初始化表达式列表”，供早期 LLVM codegen（T0811）构造值。
    pub(in crate::hir::lower) fn lower_struct_lit_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        ty: &ast::TypePath,
        fields: &[ast::StructLitField],
        expected_ty: Option<TypeId>,
    ) -> (ExprKind, TypeId) {
        // T0124: For generic structs, use the expected type (from val declaration) when the
        // struct literal's type path has no type args but the expected type is a concrete
        // instantiation of the same struct.
        let ty_id = if ty.args.is_empty() {
            if let Some(expected) = expected_ty {
                if let crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal)) =
                    self.types.kind(expected)
                {
                    if !nominal.args.is_empty() {
                        expected
                    } else {
                        self.lower_type_path(ty)
                    }
                } else {
                    self.lower_type_path(ty)
                }
            } else {
                self.lower_type_path(ty)
            }
        } else {
            self.lower_type_path(ty)
        };

        let Some((struct_fqn, concrete_args)) = self.struct_instance_from_type_id(ty_id) else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };

        let Some(info) = self.default_arg_structs.get(&struct_fqn).cloned() else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };

        let mut param_to_field: Vec<Option<usize>> = vec![None; info.params.len()];
        for (field_idx, field) in fields.iter().enumerate() {
            let field_name = field.name.text(self.source);
            let Some(param_idx) = info
                .params
                .iter()
                .position(|param| param.name == field_name)
            else {
                let lowered_fields = fields
                    .iter()
                    .map(|f| StructLitField {
                        span: f.span,
                        name: f.name.text(self.source).to_string(),
                        name_span: f.name.span,
                        colon_span: f.colon_span,
                        value: self.lower_expr(pkg_prefix, &f.value),
                    })
                    .collect::<Vec<_>>();
                return (
                    ExprKind::StructLit {
                        ty: ty_id,
                        fields: lowered_fields,
                    },
                    ty_id,
                );
            };
            let slot = param_to_field
                .get_mut(param_idx)
                .expect("param index in range");
            if slot.is_some() {
                let lowered_fields = fields
                    .iter()
                    .map(|f| StructLitField {
                        span: f.span,
                        name: f.name.text(self.source).to_string(),
                        name_span: f.name.span,
                        colon_span: f.colon_span,
                        value: self.lower_expr(pkg_prefix, &f.value),
                    })
                    .collect::<Vec<_>>();
                return (
                    ExprKind::StructLit {
                        ty: ty_id,
                        fields: lowered_fields,
                    },
                    ty_id,
                );
            }
            *slot = Some(field_idx);
        }

        let needs_defaults = param_to_field.iter().any(|slot| slot.is_none());
        if !needs_defaults {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        }

        let Some((decl_source, decl_file)) = self.decl_ast_context(&info.decl_file) else {
            let lowered_fields = fields
                .iter()
                .map(|f| StructLitField {
                    span: f.span,
                    name: f.name.text(self.source).to_string(),
                    name_span: f.name.span,
                    colon_span: f.colon_span,
                    value: self.lower_expr(pkg_prefix, &f.value),
                })
                .collect::<Vec<_>>();

            return (
                ExprKind::StructLit {
                    ty: ty_id,
                    fields: lowered_fields,
                },
                ty_id,
            );
        };
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

        let mut stmts: Vec<Stmt> = Vec::with_capacity(info.params.len() + 1);
        for field in fields {
            let field_name = field.name.text(self.source);
            let param_idx = info
                .params
                .iter()
                .position(|param| param.name == field_name)
                .expect("known field name mapped to param");
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            let init = self.lower_expr_with_expected(pkg_prefix, &field.value, expected);
            let param = info.params.get(param_idx).expect("param index in range");
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span,
                    id: Some(*param_ids.get(param_idx).expect("param id collected")),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if param_to_field.get(param_idx).copied().flatten().is_some() {
                continue;
            }
            let default_value = param
                .default_value
                .as_ref()
                .expect("missing field requires default");
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            let init = self.with_bound_struct_default_context(
                decl_source,
                decl_file,
                &info.type_params,
                &concrete_args,
                |this| this.lower_expr_with_expected(&decl_pkg_prefix, default_value, expected),
            );
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(ValDecl {
                    span,
                    id: Some(*param_ids.get(param_idx).expect("param id collected")),
                    name: Some(param.name.clone()),
                    mutable: false,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    init: Some(init),
                }),
            });
        }

        let mut lowered_fields = Vec::with_capacity(info.params.len());
        for (param_idx, param) in info.params.iter().enumerate() {
            let expected = *expecteds.get(param_idx).expect("expected info collected");
            lowered_fields.push(StructLitField {
                span,
                name: param.name.clone(),
                name_span: span,
                colon_span: span,
                value: Expr {
                    span: param.decl_span,
                    ty: expected.value_ty.unwrap_or(self.builtins.any),
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: *param_ids.get(param_idx).expect("param id collected"),
                        name: param.name.clone(),
                        decl_span: param.decl_span,
                    }),
                },
            });
        }

        let struct_expr = Expr {
            span,
            ty: ty_id,
            kind: ExprKind::StructLit {
                ty: ty_id,
                fields: lowered_fields,
            },
        };
        stmts.push(Stmt {
            span,
            ty: ty_id,
            kind: StmtKind::Expr(struct_expr),
        });

        (
            ExprKind::Block(Block {
                span,
                ty: ty_id,
                stmts,
            }),
            ty_id,
        )
    }

    /// `with` 表达式 lowering（spec §2.6）。
    ///
    /// 将 `base with { path: value }` 展开为一个 copy-update block：
    /// ```text
    /// {
    ///   val $with_base = <base>
    ///   <按具体值类型重建 aggregate>
    /// }
    /// ```
    /// 对于嵌套路径，递归重建内层 struct / tuple / enum。
    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn lower_with_update_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        base: &ast::Expr,
        updates: &[ast::WithUpdateField],
    ) -> Expr {
        let Some(contract) = self.typechecked_with_update_contract(expr_span) else {
            return self.missing_with_update_expr(expr_span);
        };

        self.with_update_contracts.insert(
            crate::hir::CallSite::new(self.source.path().to_path_buf(), expr_span),
            contract.clone(),
        );

        let aggregate_ty_map = contract
            .aggregates
            .iter()
            .map(|aggregate| (aggregate.prefix.clone(), aggregate.ty))
            .collect::<std::collections::HashMap<_, _>>();
        let aggregate_enum_map = contract
            .aggregates
            .iter()
            .filter_map(|aggregate| match &aggregate.kind {
                ast::WithUpdateAggregateContractKind::Enum { info } => {
                    Some((aggregate.prefix.clone(), info.clone()))
                }
                ast::WithUpdateAggregateContractKind::Struct { .. }
                | ast::WithUpdateAggregateContractKind::Tuple { .. } => None,
            })
            .collect::<std::collections::HashMap<_, _>>();

        let Some(base_ty) = aggregate_ty_map.get("").copied() else {
            return self.missing_with_update_expr(expr_span);
        };
        let result_ty = contract.result_ty;

        let base_lowered = self.lower_expr(pkg_prefix, base);
        let base_id = self.intern_local_symbol(with_span, false);

        let base_ref = Expr {
            span: with_span,
            ty: base_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: base_id,
                name: "$with_base".to_string(),
                decl_span: with_span,
            }),
        };

        let val_stmt = Stmt {
            span: with_span,
            ty: base_ty,
            kind: StmtKind::Val(ValDecl {
                span: with_span,
                id: Some(base_id),
                name: Some("$with_base".to_string()),
                mutable: false,
                ty: base_ty,
                init: Some(base_lowered),
            }),
        };

        let mut stmts = vec![val_stmt];
        let mut grouped: std::collections::HashMap<String, Vec<WithUpdateGroupedValue>> =
            std::collections::HashMap::new();
        for u in updates {
            let segs = &u.path.segments;
            if segs.is_empty() {
                continue;
            }
            let lowered_value = self.lower_expr(pkg_prefix, &u.value);
            let value_ty = lowered_value.ty;
            let (decl_span, value_id, value_name) =
                self.fresh_synthetic_local(u.value.span, "__with_update_value", false);
            stmts.push(Stmt {
                span: u.value.span,
                ty: value_ty,
                kind: StmtKind::Val(ValDecl {
                    span: u.value.span,
                    id: Some(value_id),
                    name: Some(value_name.clone()),
                    mutable: false,
                    ty: value_ty,
                    init: Some(lowered_value),
                }),
            });
            let value_ref = Expr {
                span: u.value.span,
                ty: value_ty,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: value_id,
                    name: value_name,
                    decl_span,
                }),
            };
            let first = self.source.slice(segs[0].span).to_string();
            grouped
                .entry(first)
                .or_default()
                .push(WithUpdateGroupedValue {
                    rest: segs[1..].to_vec(),
                    value: value_ref,
                });
        }

        let rebuilt = self.build_with_copy_expr(
            pkg_prefix,
            expr_span,
            with_span,
            base_ty,
            &base_ref,
            &grouped,
            &aggregate_ty_map,
            &aggregate_enum_map,
            "",
        );

        let result_stmt = Stmt {
            span: expr_span,
            ty: result_ty,
            kind: StmtKind::Expr(rebuilt),
        };
        stmts.push(result_stmt);

        Expr {
            span: expr_span,
            ty: result_ty,
            kind: ExprKind::Block(Block {
                span: expr_span,
                ty: result_ty,
                stmts,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn build_with_copy_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        aggregate_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<WithUpdateGroupedValue>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        enum LoweringAggregateKind {
            Struct(String),
            Tuple,
            Enum,
            Unsupported,
        }

        let lowering_kind = match self.types.kind(aggregate_ty) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if matches!(
                    self.type_kinds.get(&nominal.fqn),
                    Some(&ast::TypeKind::Struct)
                ) =>
            {
                LoweringAggregateKind::Struct(nominal.fqn.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(_)) => LoweringAggregateKind::Tuple,
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if matches!(
                    self.type_kinds.get(&nominal.fqn),
                    Some(&ast::TypeKind::Enum)
                ) =>
            {
                LoweringAggregateKind::Enum
            }
            _ => LoweringAggregateKind::Unsupported,
        };

        match lowering_kind {
            LoweringAggregateKind::Struct(struct_fqn) => self.build_with_struct_lit(
                pkg_prefix,
                expr_span,
                with_span,
                &struct_fqn,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Tuple => self.build_with_tuple_lit(
                pkg_prefix,
                expr_span,
                with_span,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Enum => self.build_with_enum_expr(
                pkg_prefix,
                expr_span,
                with_span,
                aggregate_ty,
                base_access,
                grouped,
                aggregate_ty_map,
                aggregate_enum_map,
                current_prefix,
            ),
            LoweringAggregateKind::Unsupported => self.missing_with_update_expr(expr_span),
        }
    }

    /// 递归构造 with-update 的 StructLit 表达式。
    ///
    /// `base_access` 是访问当前层级 base 值的表达式（例如 `$with_base` 或 `$with_base.start`）。
    /// `grouped` 中 key 为当前层级的 field name，value 为 (remaining path segments, value expr)。
    /// `aggregate_ty_map` 为 typecheck 写回的 path_prefix → 具体 aggregate type 映射。
    /// `current_prefix` 为当前层级的路径前缀（例如 `""` 或 `"start"`）。
    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn build_with_struct_lit(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        struct_fqn: &str,
        struct_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<WithUpdateGroupedValue>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let field_names: Vec<String> = self
            .index
            .constructors
            .get(struct_fqn)
            .and_then(|ctors| {
                ctors
                    .iter()
                    .find(|c| c.kind == crate::resolve::ConstructorKind::Primary)
            })
            .map(|ctor| ctor.params.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();

        let mut fields = Vec::with_capacity(field_names.len());

        for field_name in &field_names {
            let field_fqn = format!("{}.{}", struct_fqn, field_name);
            let field_id = self.symbols.intern_top_level(field_fqn.clone());
            let field_access = Expr {
                span: with_span,
                ty: self.builtins.any,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(base_access.clone()),
                    member: MemberAccess {
                        span: with_span,
                        name: field_name.clone(),
                        resolved: Some(MemberRef::Value {
                            id: field_id,
                            fqn: field_fqn,
                        }),
                    },
                },
            };

            let value = if let Some(update_group) = grouped.get(field_name) {
                self.build_with_field_value(
                    pkg_prefix,
                    expr_span,
                    with_span,
                    field_name,
                    field_access,
                    update_group,
                    aggregate_ty_map,
                    aggregate_enum_map,
                    current_prefix,
                )
            } else {
                field_access
            };

            fields.push(StructLitField {
                span: with_span,
                name: field_name.clone(),
                name_span: with_span,
                colon_span: with_span,
                value,
            });
        }

        Expr {
            span: expr_span,
            ty: struct_ty,
            kind: ExprKind::StructLit {
                ty: struct_ty,
                fields,
            },
        }
    }

    /// 递归构造 with-update 的 TupleLit 表达式。
    ///
    /// tuple 元素沿用 `0` / `1` / ... 成员访问语法读取原值，再按 grouped 中的更新重建。
    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn build_with_tuple_lit(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        tuple_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<WithUpdateGroupedValue>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let element_tys = match self.types.kind(tuple_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.to_vec(),
            _ => {
                return self.missing_with_update_expr(expr_span);
            }
        };

        let mut elements = Vec::with_capacity(element_tys.len());
        for (idx, _) in element_tys.iter().enumerate() {
            let member_name = idx.to_string();
            let field_access = Expr {
                span: with_span,
                ty: self.builtins.any,
                kind: ExprKind::MemberAccess {
                    receiver: Box::new(base_access.clone()),
                    member: MemberAccess {
                        span: with_span,
                        name: member_name.clone(),
                        resolved: None,
                    },
                },
            };

            let value = if let Some(update_group) = grouped.get(&member_name) {
                self.build_with_field_value(
                    pkg_prefix,
                    expr_span,
                    with_span,
                    &member_name,
                    field_access,
                    update_group,
                    aggregate_ty_map,
                    aggregate_enum_map,
                    current_prefix,
                )
            } else {
                field_access
            };

            elements.push(value);
        }

        Expr {
            span: expr_span,
            ty: tuple_ty,
            kind: ExprKind::TupleLit { elements },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn build_with_enum_expr(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        enum_ty: TypeId,
        base_access: &Expr,
        grouped: &std::collections::HashMap<String, Vec<WithUpdateGroupedValue>>,
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        let Some(enum_info) = aggregate_enum_map.get(current_prefix) else {
            return self.missing_with_update_expr(expr_span);
        };

        let mut arms: Vec<WhenArm> = Vec::with_capacity(enum_info.variants.len());
        for variant in &enum_info.variants {
            let update_group = grouped.get(&variant.name);
            let mut pat_args: Vec<WhenPat> = Vec::with_capacity(variant.fields.len());
            let mut field_refs: Vec<(String, Expr)> = Vec::with_capacity(variant.fields.len());

            for field in &variant.fields {
                let (decl_span, id, name) =
                    self.fresh_synthetic_local(with_span, "__with_enum_field", false);
                self.record_when_pat_binding_ty(decl_span, field.ty);
                pat_args.push(WhenPat::Bind {
                    span: decl_span,
                    id,
                    name: name.clone(),
                });
                field_refs.push((
                    field.name.clone(),
                    Expr {
                        span: with_span,
                        ty: field.ty,
                        kind: ExprKind::VarRef(ValueRef::Local {
                            id,
                            name,
                            decl_span,
                        }),
                    },
                ));
            }

            let body = if let Some(update_group) = update_group {
                let mut grouped_by_field: std::collections::HashMap<
                    String,
                    Vec<WithUpdateGroupedValue>,
                > = std::collections::HashMap::new();
                for update in update_group {
                    if update.rest.is_empty() {
                        return self.missing_with_update_expr(expr_span);
                    }
                    let next = self.source.slice(update.rest[0].span).to_string();
                    grouped_by_field
                        .entry(next)
                        .or_default()
                        .push(WithUpdateGroupedValue {
                            rest: update.rest[1..].to_vec(),
                            value: update.value.clone(),
                        });
                }

                let mut args: Vec<CallArg> = Vec::with_capacity(variant.fields.len());
                for field in &variant.fields {
                    let Some((_, field_ref)) =
                        field_refs.iter().find(|(name, _)| name == &field.name)
                    else {
                        return self.missing_with_update_expr(expr_span);
                    };
                    let variant_prefix = if current_prefix.is_empty() {
                        variant.name.clone()
                    } else {
                        format!("{}.{}", current_prefix, variant.name)
                    };
                    let value = if let Some(field_group) = grouped_by_field.get(&field.name) {
                        self.build_with_field_value(
                            pkg_prefix,
                            expr_span,
                            with_span,
                            &field.name,
                            field_ref.clone(),
                            field_group,
                            aggregate_ty_map,
                            aggregate_enum_map,
                            &variant_prefix,
                        )
                    } else {
                        field_ref.clone()
                    };
                    args.push(CallArg::Positional(value));
                }

                Expr {
                    span: expr_span,
                    ty: enum_ty,
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            span: with_span,
                            ty: self.builtins.any,
                            kind: ExprKind::UnresolvedIdent {
                                name: variant.name.clone(),
                            },
                        }),
                        args,
                    },
                }
            } else {
                base_access.clone()
            };

            arms.push(WhenArm {
                span: expr_span,
                pat: WhenPat::Variant {
                    span: with_span,
                    name_span: with_span,
                    name: variant.name.clone(),
                    args: pat_args,
                },
                guard: None,
                arrow_span: with_span,
                body,
            });
        }

        Expr {
            span: expr_span,
            ty: enum_ty,
            kind: ExprKind::When {
                subject: Box::new(base_access.clone()),
                arms,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir::lower) fn build_with_field_value(
        &mut self,
        pkg_prefix: &str,
        expr_span: Span,
        with_span: Span,
        field_name: &str,
        field_access: Expr,
        update_group: &[WithUpdateGroupedValue],
        aggregate_ty_map: &std::collections::HashMap<String, TypeId>,
        aggregate_enum_map: &std::collections::HashMap<String, ast::WithUpdateResolvedEnum>,
        current_prefix: &str,
    ) -> Expr {
        if let Some(update) = update_group.iter().find(|update| update.rest.is_empty()) {
            return update.value.clone();
        }

        let nested_prefix = if current_prefix.is_empty() {
            field_name.to_string()
        } else {
            format!("{}.{}", current_prefix, field_name)
        };

        let Some(nested_ty) = aggregate_ty_map.get(&nested_prefix).copied() else {
            return self.missing_with_update_expr(expr_span);
        };

        let mut nested_grouped: std::collections::HashMap<String, Vec<WithUpdateGroupedValue>> =
            std::collections::HashMap::new();
        for update in update_group {
            if !update.rest.is_empty() {
                let next = self.source.slice(update.rest[0].span).to_string();
                nested_grouped
                    .entry(next)
                    .or_default()
                    .push(WithUpdateGroupedValue {
                        rest: update.rest[1..].to_vec(),
                        value: update.value.clone(),
                    });
            }
        }

        self.build_with_copy_expr(
            pkg_prefix,
            expr_span,
            with_span,
            nested_ty,
            &field_access,
            &nested_grouped,
            aggregate_ty_map,
            aggregate_enum_map,
            &nested_prefix,
        )
    }
}
