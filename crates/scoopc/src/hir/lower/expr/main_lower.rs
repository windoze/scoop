//! Main lower_expr / lower_expr_with_expected dispatch and range lowering.

#![allow(dead_code)]

use super::*;

impl<'a> HirLowering<'a> {
    pub(in crate::hir::lower) fn invalid_expr_kind_after_stage_error(
        &mut self,
        _span: Span,
    ) -> (ExprKind, TypeId) {
        (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit)
    }

    pub(in crate::hir::lower) fn invalid_expr_after_stage_error(&mut self, span: Span) -> Expr {
        let (kind, ty) = self.invalid_expr_kind_after_stage_error(span);
        Expr { span, ty, kind }
    }

    fn scalar_operator_owner_fqn(&self, ty: TypeId) -> Option<String> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Bool) => Some("scoop.core.Bool".to_string()),
            TypeKind::Value(ValueTypeKind::Char) => Some("scoop.core.Char".to_string()),
            TypeKind::Value(ValueTypeKind::Float64) => Some("scoop.core.Float64".to_string()),
            TypeKind::Value(ValueTypeKind::Float32) => Some("scoop.core.Float32".to_string()),
            TypeKind::Value(ValueTypeKind::Int) => Some("scoop.core.Int".to_string()),
            TypeKind::Value(ValueTypeKind::UInt) => Some("scoop.core.UInt".to_string()),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(format!("scoop.core.Int{bits}")),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(format!("scoop.core.UInt{bits}")),
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn is_float_type(&self, ty: TypeId) -> bool {
        ty == self.builtins.float64
            || ty == self.builtins.float32
            || matches!(
                self.types.kind(ty),
                TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
            )
    }

    fn ast_int_literal_absorbs_to(&self, expr: &ast::Expr, ty: TypeId) -> bool {
        matches!(expr.kind, ast::ExprKind::IntLit) && self.is_integer_type(ty)
    }

    fn ast_float_literal_absorbs_to(&self, expr: &ast::Expr, ty: TypeId) -> bool {
        matches!(expr.kind, ast::ExprKind::FloatLit) && self.is_float_type(ty)
    }

    fn unified_integer_operator_ty(
        &self,
        lhs: &ast::Expr,
        lhs_ty: TypeId,
        rhs: &ast::Expr,
        rhs_ty: TypeId,
    ) -> Option<TypeId> {
        if lhs_ty == rhs_ty && self.is_integer_type(lhs_ty) {
            return Some(lhs_ty);
        }
        if self.ast_int_literal_absorbs_to(lhs, rhs_ty) {
            return Some(rhs_ty);
        }
        if self.ast_int_literal_absorbs_to(rhs, lhs_ty) {
            return Some(lhs_ty);
        }
        None
    }

    fn unified_float_operator_ty(
        &self,
        lhs: &ast::Expr,
        lhs_ty: TypeId,
        rhs: &ast::Expr,
        rhs_ty: TypeId,
    ) -> Option<TypeId> {
        if lhs_ty == rhs_ty && self.is_float_type(lhs_ty) {
            return Some(lhs_ty);
        }
        if self.ast_float_literal_absorbs_to(lhs, rhs_ty) {
            return Some(rhs_ty);
        }
        if self.ast_float_literal_absorbs_to(rhs, lhs_ty) {
            return Some(lhs_ty);
        }
        None
    }

    fn lower_operator_method_call_from_receiver(
        &mut self,
        span: Span,
        receiver: Expr,
        receiver_ty: TypeId,
        method: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Option<Expr> {
        let owner_fqn = self.scalar_operator_owner_fqn(receiver_ty)?;
        let method_fqn = format!("{owner_fqn}.{method}");
        Some(self.lower_synthetic_member_call_with_receiver_ty(
            span,
            receiver,
            receiver_ty,
            &method_fqn,
            args,
            ret_ty,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_operator_method_call(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver_ast: &ast::Expr,
        receiver_ty: TypeId,
        method: &str,
        arg_ast: &ast::Expr,
        arg_ty: TypeId,
        ret_ty: TypeId,
    ) -> Option<Expr> {
        let receiver_expected = self.expected_expr_for_param_ty(receiver_ty);
        let receiver = self.lower_expr_with_expected(pkg_prefix, receiver_ast, receiver_expected);
        let arg_expected = self.expected_expr_for_param_ty(arg_ty);
        let arg = self.lower_expr_with_expected(pkg_prefix, arg_ast, arg_expected);
        self.lower_operator_method_call_from_receiver(
            span,
            receiver,
            receiver_ty,
            method,
            vec![arg],
            ret_ty,
        )
    }

    fn lower_bool_not_call(&mut self, span: Span, operand: Expr) -> Option<Expr> {
        self.lower_operator_method_call_from_receiver(
            span,
            operand,
            self.builtins.bool_,
            "not",
            Vec::new(),
            self.builtins.bool_,
        )
    }

    fn lower_equals_then_not(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        lhs_ty: TypeId,
        rhs: &ast::Expr,
        rhs_ty: TypeId,
    ) -> Option<Expr> {
        let equals_span = self.fresh_synthetic_call_site_span(span);
        let equals = self.lower_operator_method_call(
            pkg_prefix,
            equals_span,
            lhs,
            lhs_ty,
            "equals",
            rhs,
            rhs_ty,
            self.builtins.bool_,
        )?;
        self.lower_bool_not_call(span, equals)
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_compare_to_operator_call(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        lhs_ty: TypeId,
        rhs: &ast::Expr,
        rhs_ty: TypeId,
        method: &str,
    ) -> Option<Expr> {
        let compare_span = self.fresh_synthetic_call_site_span(span);
        let compare_to = self.lower_operator_method_call(
            pkg_prefix,
            compare_span,
            lhs,
            lhs_ty,
            "compareTo",
            rhs,
            rhs_ty,
            self.builtins.int,
        )?;
        let zero = Expr {
            span: self.fresh_synthetic_call_site_span(span),
            ty: self.builtins.int,
            kind: ExprKind::Literal(LiteralKind::SynthInt(0)),
        };
        self.lower_operator_method_call_from_receiver(
            span,
            compare_to,
            self.builtins.int,
            method,
            vec![zero],
            self.builtins.bool_,
        )
    }

    fn try_lower_builtin_unary_operator_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op: ast::UnaryOp,
        expr: &ast::Expr,
    ) -> Option<Expr> {
        let operand_ty = self.typechecked_expr_ty(expr.span)?;
        let (method, ret_ty) = match op {
            ast::UnaryOp::Not if operand_ty == self.builtins.bool_ => ("not", self.builtins.bool_),
            ast::UnaryOp::Neg
                if self.is_integer_type(operand_ty) || self.is_float_type(operand_ty) =>
            {
                ("unaryMinus", operand_ty)
            }
            ast::UnaryOp::BitNot if self.is_integer_type(operand_ty) => ("inv", operand_ty),
            _ => return None,
        };
        let operand_expected = self.expected_expr_for_param_ty(operand_ty);
        let operand = self.lower_expr_with_expected(pkg_prefix, expr, operand_expected);
        self.lower_operator_method_call_from_receiver(
            span,
            operand,
            operand_ty,
            method,
            Vec::new(),
            ret_ty,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn try_lower_builtin_binary_operator_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op: ast::BinaryOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<Expr> {
        if matches!(
            op,
            ast::BinaryOp::LogAnd
                | ast::BinaryOp::LogOr
                | ast::BinaryOp::RangeInclusive
                | ast::BinaryOp::Elvis
        ) {
            return None;
        }

        let lhs_ty = self.typechecked_expr_ty(lhs.span)?;
        let rhs_ty = self.typechecked_expr_ty(rhs.span)?;
        let result_ty = self.typechecked_expr_ty(span).unwrap_or(self.builtins.any);

        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => {
                let method = match op {
                    ast::BinaryOp::Add => "plus",
                    ast::BinaryOp::Sub => "minus",
                    ast::BinaryOp::Mul => "times",
                    ast::BinaryOp::Div => "div",
                    ast::BinaryOp::Rem => "rem",
                    ast::BinaryOp::BitAnd => "and",
                    ast::BinaryOp::BitXor => "xor",
                    ast::BinaryOp::BitOr => "or",
                    _ => unreachable!("filtered by outer match"),
                };
                if lhs_ty == self.builtins.bool_ && rhs_ty == self.builtins.bool_ {
                    return self.lower_operator_method_call(
                        pkg_prefix,
                        span,
                        lhs,
                        lhs_ty,
                        method,
                        rhs,
                        rhs_ty,
                        self.builtins.bool_,
                    );
                }
                if lhs_ty == self.builtins.char_
                    && matches!(op, ast::BinaryOp::Add | ast::BinaryOp::Sub)
                    && (rhs_ty == self.builtins.int || rhs_ty == self.builtins.char_)
                {
                    return self.lower_operator_method_call(
                        pkg_prefix, span, lhs, lhs_ty, method, rhs, rhs_ty, result_ty,
                    );
                }
                if let Some(operand_ty) = self
                    .unified_integer_operator_ty(lhs, lhs_ty, rhs, rhs_ty)
                    .or_else(|| self.unified_float_operator_ty(lhs, lhs_ty, rhs, rhs_ty))
                {
                    return self.lower_operator_method_call(
                        pkg_prefix, span, lhs, operand_ty, method, rhs, operand_ty, operand_ty,
                    );
                }
            }
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if self.is_integer_type(lhs_ty) && rhs_ty == self.builtins.int {
                    let method = match op {
                        ast::BinaryOp::Shl => "shl",
                        ast::BinaryOp::Shr => "shr",
                        _ => unreachable!("filtered by outer match"),
                    };
                    return self.lower_operator_method_call(
                        pkg_prefix,
                        span,
                        lhs,
                        lhs_ty,
                        method,
                        rhs,
                        self.builtins.int,
                        lhs_ty,
                    );
                }
            }
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                let method = match op {
                    ast::BinaryOp::Lt => "lt",
                    ast::BinaryOp::Le => "le",
                    ast::BinaryOp::Gt => "gt",
                    ast::BinaryOp::Ge => "ge",
                    _ => unreachable!("filtered by outer match"),
                };
                if let Some(operand_ty) = self
                    .unified_integer_operator_ty(lhs, lhs_ty, rhs, rhs_ty)
                    .or_else(|| self.unified_float_operator_ty(lhs, lhs_ty, rhs, rhs_ty))
                {
                    return self.lower_operator_method_call(
                        pkg_prefix,
                        span,
                        lhs,
                        operand_ty,
                        method,
                        rhs,
                        operand_ty,
                        self.builtins.bool_,
                    );
                }
                if lhs_ty == self.builtins.char_ && rhs_ty == self.builtins.char_ {
                    return self.lower_compare_to_operator_call(
                        pkg_prefix, span, lhs, lhs_ty, rhs, rhs_ty, method,
                    );
                }
            }
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if lhs_ty == self.builtins.bool_ && rhs_ty == self.builtins.bool_ {
                    let method = match op {
                        ast::BinaryOp::Eq => "equals",
                        ast::BinaryOp::Ne => "notEquals",
                        _ => unreachable!("filtered by outer match"),
                    };
                    return self.lower_operator_method_call(
                        pkg_prefix,
                        span,
                        lhs,
                        lhs_ty,
                        method,
                        rhs,
                        rhs_ty,
                        self.builtins.bool_,
                    );
                }
                if let Some(operand_ty) = self
                    .unified_integer_operator_ty(lhs, lhs_ty, rhs, rhs_ty)
                    .or_else(|| self.unified_float_operator_ty(lhs, lhs_ty, rhs, rhs_ty))
                {
                    let method = match op {
                        ast::BinaryOp::Eq => "equals",
                        ast::BinaryOp::Ne => "notEquals",
                        _ => unreachable!("filtered by outer match"),
                    };
                    return self.lower_operator_method_call(
                        pkg_prefix,
                        span,
                        lhs,
                        operand_ty,
                        method,
                        rhs,
                        operand_ty,
                        self.builtins.bool_,
                    );
                }
                if lhs_ty == self.builtins.char_ && rhs_ty == self.builtins.char_ {
                    return if op == ast::BinaryOp::Eq {
                        self.lower_operator_method_call(
                            pkg_prefix,
                            span,
                            lhs,
                            lhs_ty,
                            "equals",
                            rhs,
                            rhs_ty,
                            self.builtins.bool_,
                        )
                    } else {
                        self.lower_equals_then_not(pkg_prefix, span, lhs, lhs_ty, rhs, rhs_ty)
                    };
                }
            }
            ast::BinaryOp::RangeInclusive
            | ast::BinaryOp::LogAnd
            | ast::BinaryOp::LogOr
            | ast::BinaryOp::Elvis => {}
        }

        None
    }

    pub(in crate::hir::lower) fn lower_expr(&mut self, pkg_prefix: &str, e: &ast::Expr) -> Expr {
        self.lower_expr_with_expected(pkg_prefix, e, ExpectedExpr::default())
    }

    /// lowering 表达式并携带“期望类型 hint”。
    ///
    /// 注意：该 hint 仅用于把 `[...]` 降到稳定的 builder/intrinsics 调用形态（TODO T1317c），
    /// 不等价于完整 typecheck 的 expected-type 推断。
    pub(in crate::hir::lower) fn lower_expr_with_expected(
        &mut self,
        pkg_prefix: &str,
        e: &ast::Expr,
        expected: ExpectedExpr,
    ) -> Expr {
        let (kind, ty) = match &e.kind {
            ast::ExprKind::Missing => {
                self.record_stage_error(
                    e.span,
                    "parser recovery Missing expression cannot enter HIR lowering",
                    "HIR expression lowering",
                );
                self.invalid_expr_kind_after_stage_error(e.span)
            }
            ast::ExprKind::Annotated { expr, .. } => {
                return self.lower_expr_with_expected(pkg_prefix, expr, expected);
            }
            ast::ExprKind::IntLit => {
                let ty = expected
                    .value_ty
                    .filter(|ty| self.is_integer_type(*ty))
                    .or_else(|| {
                        self.typechecked_expr_ty(e.span)
                            .filter(|ty| self.is_integer_type(*ty))
                    })
                    .unwrap_or(self.builtins.int);
                (ExprKind::Literal(LiteralKind::Int), ty)
            }
            ast::ExprKind::FloatLit => {
                let parsed = parse_float_literal(self.source.slice(e.span));
                if expected.value_ty == Some(self.builtins.float32)
                    || self.typechecked_expr_ty(e.span) == Some(self.builtins.float32)
                {
                    (
                        ExprKind::Literal(LiteralKind::Float32(parsed.value as f32)),
                        self.builtins.float32,
                    )
                } else if expected.value_ty == Some(self.builtins.float64) {
                    (
                        ExprKind::Literal(LiteralKind::Float64(parsed.value)),
                        self.builtins.float64,
                    )
                } else {
                    match parsed.suffix {
                        FloatLiteralSuffix::Float64 => (
                            ExprKind::Literal(LiteralKind::Float64(parsed.value)),
                            self.builtins.float64,
                        ),
                        FloatLiteralSuffix::Float32 => (
                            ExprKind::Literal(LiteralKind::Float32(parsed.value as f32)),
                            self.builtins.float32,
                        ),
                    }
                }
            }
            ast::ExprKind::CharLit => {
                let value = parse_char_literal(self.source.slice(e.span))
                    .expect("lexer validated Char literal before HIR lowering");
                (
                    ExprKind::Literal(LiteralKind::Char(value)),
                    self.builtins.char_,
                )
            }
            ast::ExprKind::StringLit => {
                (ExprKind::Literal(LiteralKind::String), self.builtins.string)
            }
            ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit),
            ast::ExprKind::ArrayLit { elements } => {
                if let Some((target, result_ty, element_expected_ty)) =
                    self.array_lit_lowering_hint(e.span, expected)
                {
                    self.lower_array_lit_expr(
                        pkg_prefix,
                        e.span,
                        elements,
                        target,
                        result_ty,
                        element_expected_ty,
                    )
                } else {
                    let lowered_elements: Vec<Expr> = elements
                        .iter()
                        .map(|element| self.lower_expr(pkg_prefix, element))
                        .collect();
                    match self.infer_array_lit_ty_from_lowered_elements(&lowered_elements) {
                        Some(result_ty) => self.build_array_lit_expr(
                            e.span,
                            lowered_elements,
                            ArrayLitTarget::Array,
                            result_ty,
                        ),
                        None => {
                            let result_ty = self.intern_nominal(
                                "scoop.core.Array".to_string(),
                                vec![self.builtins.any],
                                None,
                            );
                            self.build_array_lit_expr(
                                e.span,
                                lowered_elements,
                                ArrayLitTarget::Array,
                                result_ty,
                            )
                        }
                    }
                }
            }
            ast::ExprKind::ClassLit { ty } => {
                let source_ty = self.lower_type_ref(ty);
                let source_fqn = self
                    .index
                    .type_ref_to_fqn_in_file(self.source, self.file, ty);
                (
                    ExprKind::ClassLiteral(ClassLiteralExpr {
                        source_ty,
                        source_fqn,
                        metadata_kind: TypeMetadataLiteralKind::TypeNameString,
                        result_ty: self.builtins.string,
                    }),
                    self.builtins.string,
                )
            }
            ast::ExprKind::InterpolatedString { raw, parts } => {
                let expr = self.desugar_f_string_expr(pkg_prefix, e.span, *raw, parts);
                return expr;
            }
            ast::ExprKind::Ident(id) => self
                .try_lower_top_level_fun_value_expr(e, expected)
                .unwrap_or_else(|| self.lower_ident_expr(id)),
            ast::ExprKind::Block(b) => {
                let b = self.lower_block_with_expected(pkg_prefix, b, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::DoBlock { body, .. } => {
                // `do { ... }` 在 HIR 层面与普通 block 表达式等价。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::UnsafeBlock { body, .. } => {
                // `@Unsafe do { ... }` 仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1004）。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::SafeBlock { body, .. } => {
                // `@Safe do { ... }` 同样仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1021）。
                let b = self.lower_block_with_expected(pkg_prefix, body, expected);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::TypeApply { callee, .. } => self
                .try_lower_top_level_fun_value_expr(e, expected)
                .unwrap_or_else(|| {
                    // v0：HIR 暂不承载显式类型实参；先把它视为 callee 的透明包装。
                    // 反射 intrinsics 的 type args 语义目前由 comptime 解释器消费（T1204）。
                    let inner = self.lower_expr(pkg_prefix, callee);
                    (inner.kind, inner.ty)
                }),
            ast::ExprKind::Call { callee, args } => {
                // 调用表达式在 typecheck 后已经有稳定结果类型；这里即使后续把 member/extension/default-arg
                // 调用降糖成其它 HIR 形态，也要优先保留该结果类型，避免局部 `val x = call(...)`
                // 因为中间表达式被写成 `Any` 而在 codegen 时触发错误的 value coercion。
                let typechecked_call_ty = self.typechecked_expr_ty(e.span);
                let call_ty = typechecked_call_ty.unwrap_or(self.builtins.any);
                let callee_expr = self.transparent_call_callee(callee);
                let synthesized_args =
                    self.synthesized_unit_call_args_for_typed_sugar(e.span, args);
                let args = synthesized_args.as_deref().unwrap_or(args.as_slice());

                // T0108：safe call 方法调用：`receiver?.method(args)` → when desugar。
                if let ast::ExprKind::SafeMemberAccess {
                    receiver: inner_receiver,
                    op_span,
                    member,
                } = &callee_expr.kind
                {
                    let (kind, ty) = self.lower_safe_call_expr(
                        pkg_prefix,
                        e.span,
                        inner_receiver,
                        *op_span,
                        member,
                        args,
                    );
                    return Expr {
                        span: e.span,
                        ty: typechecked_call_ty.unwrap_or(ty),
                        kind,
                    };
                }

                // 扩展函数调用（T0312）：把 `receiver.ext(args...)` 降糖为普通顶层调用：
                // `ext(receiver, args...)`。
                //
                // 说明：
                // - 运行期 codegen 当前只直接支持 `TopLevel` callee（以及少量特殊 member call）；
                // - 这里在 lowering 阶段提前把 extension call 改写为顶层调用，避免后端无法识别 `MemberAccess` callee。
                if let Some((kind, ty)) = (|| {
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee_expr.kind else {
                        return None;
                    };
                    let resolved = self.resolved_member_for_lowering(member);
                    let ast::ResolvedMemberRef::ExtensionFun { fqn } = resolved.as_ref()? else {
                        return None;
                    };

                    let overload = self.fun_overload_by_fqn(fqn);
                    // expected-type hint 目前只用于数组字面量 `[...]` 的 lowering（Array vs MutableArray）。
                    // receiver 不是数组字面量时无需解析签名里的 receiver TypeRef；若需要读取 imported/sysroot
                    // 签名，则必须切回声明源文件上下文，不能再把 foreign span 当成 caller 文件来切片。
                    let receiver_is_array_lit =
                        matches!(receiver.kind, ast::ExprKind::ArrayLit { .. });
                    let receiver_expected = ExpectedExpr {
                        value_ty: None,
                        array_lit_target: match receiver_is_array_lit {
                            true => {
                                if let Some(overload) = overload.as_ref() {
                                    if let Some(receiver_ty) = overload.sig.receiver.as_ref() {
                                        self.array_lit_target_from_type_ref_in_decl_context(
                                            &overload.symbol.decl_file,
                                            receiver_ty,
                                        )
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            false => None,
                        },
                        array_lit_ty: None,
                        struct_lit_ty: None,
                    };
                    let receiver =
                        self.lower_expr_with_expected(pkg_prefix, receiver, receiver_expected);

                    if let Some(arg_binding) = self.typechecked_call_arg_binding(e.span)
                        && let Some(fun_binding) =
                            self.typechecked_top_level_fun_call_binding(e.span)
                    {
                        let target_fqn = self
                            .materialized_direct_call_target_fqn_for_binding(&fun_binding)
                            .unwrap_or_else(|| fqn.clone());
                        let callee = self.top_level_callee_expr_with_fqn(callee.span, target_fqn);
                        let plan = self.callable_param_plan_for_fun_binding(&fun_binding);
                        if let Some((kind, ty)) =
                            self.lower_canonical_call_expr(CanonicalCallLoweringRequest {
                                pkg_prefix,
                                call_span: e.span,
                                callee,
                                source_args: args,
                                receiver: Some(receiver.clone()),
                                binding: arg_binding,
                                plan,
                                call_ty,
                            })
                        {
                            return Some((kind, ty));
                        }
                    }

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    let mut positional_index = 0usize;
                    for arg in args {
                        let expected = self.expected_expr_for_fun_call_arg(
                            overload.as_ref(),
                            arg,
                            positional_index,
                        );
                        if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                            positional_index = positional_index.saturating_add(1);
                        }
                        lowered_args
                            .push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                    }

                    let target_fqn = self
                        .materialized_top_level_fun_call_target_fqn(e.span)
                        .unwrap_or_else(|| fqn.clone());
                    let callee = self.top_level_callee_expr_with_fqn(callee.span, target_fqn);

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        call_ty,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) =
                    self.try_lower_effect_op_call_expr(pkg_prefix, e.span, callee, args)
                {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else if let Some((kind, ty)) = (|| {
                    // T1508a：直连成员函数调用（final/private）：把 `receiver.method(args...)`
                    // 降糖为顶层调用 `Owner.method(receiver, args...)`。
                    //
                    // 注意：
                    // - value receiver 直接沿用原 receiver lowering；
                    // - 对 `TypeName.member(...)` 这类 companion dispatch，receiver 在 AST/typecheck 中
                    //   仍是“未 resolve 的类型名 ident”，这里需要显式改写成 companion object 单例值，
                    //   才能进入和普通 object member call 相同的 direct-call 主线；
                    // - `GC.pin/unpin` / `GC.handle*` 这组 sysroot intrinsic member call 具有专门的
                    //   MIR/runtime contract；必须保留 member-access callee 形状，不能在这里改写为顶层调用。
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee_expr.kind else {
                        return None;
                    };
                    if self.should_keep_member_call_as_member_access(receiver, member) {
                        return None;
                    }
                    let resolved = self.resolved_member_for_lowering(member);
                    let ast::ResolvedMemberRef::Fun { fqn } = resolved.as_ref()? else {
                        return None;
                    };
                    let overload = self.fun_overload_by_fqn(fqn);
                    let member_call_ty = typechecked_call_ty
                        .or_else(|| self.fun_overload_return_ty(overload.as_ref()))
                        .unwrap_or(call_ty);

                    if fqn == "scoop.core.GC.pin"
                        || fqn == "scoop.core.GC.unpin"
                        || fqn == "scoop.core.GC.handleNew"
                        || fqn == "scoop.core.GC.handleGet"
                        || fqn == "scoop.core.GC.handleDrop"
                    {
                        return None;
                    }

                    // 统一把 ordinary member call 降糖为顶层调用：
                    // `receiver.method(args...)` -> `Owner.method(receiver, args...)`。
                    // 这里覆盖 struct/class/interface/object；内建 by-name keep-list 仍由上面的
                    // `should_keep_member_call_as_member_access` 控制。
                    let (owner_fqn, member_name) = fqn.as_str().rsplit_once('.')?;
                    let owner_is_struct =
                        matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Struct));
                    let owner_is_class =
                        matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class));
                    let owner_is_interface = matches!(
                        self.type_kinds.get(owner_fqn),
                        Some(ast::TypeKind::Interface)
                    );
                    let owner_is_object = self.index.object_types.contains(owner_fqn);
                    if !owner_is_struct
                        && !owner_is_class
                        && !owner_is_interface
                        && !owner_is_object
                    {
                        return None;
                    }

                    let receiver = if let ast::ExprKind::Ident(id) = &receiver.kind
                        && id.resolved.is_none()
                        && self.source.slice(id.span) != "this"
                    {
                        if !owner_is_object {
                            return None;
                        }
                        self.synth_object_singleton_value_expr(owner_fqn, receiver.span)
                    } else {
                        self.lower_expr(pkg_prefix, receiver)
                    };
                    // `String` is an intrinsic final runtime type; it has no object vtable, so its
                    // body methods must lower as direct calls just like intrinsic scalar structs.
                    let owner_uses_virtual_dispatch =
                        owner_is_class && owner_fqn != "scoop.core.String";

                    if let Some(arg_binding) = self.typechecked_call_arg_binding(e.span)
                        && let Some(fun_binding) =
                            self.typechecked_top_level_fun_call_binding(e.span)
                    {
                        let receiver_ty = receiver.ty;
                        let dispatch_kind = if owner_is_interface {
                            Some(crate::hir::DispatchCallKind::Interface)
                        } else if owner_uses_virtual_dispatch {
                            Some(crate::hir::DispatchCallKind::Virtual)
                        } else {
                            None
                        };
                        let target_fqn = if let Some(dispatch_kind) = dispatch_kind {
                            if self.devirtualize_dispatch_calls {
                                if let Some(target_fqn) =
                                    crate::devirtualize::try_devirtualize_dispatch_target(
                                        dispatch_kind,
                                        owner_fqn,
                                        member_name,
                                        args.len(),
                                        receiver_ty,
                                        self.types,
                                        crate::devirtualize::DispatchTargetFacts {
                                            known_receiver_subclasses: self
                                                .known_receiver_subclasses,
                                            class_vtables: self.class_vtables,
                                            interfaces: self.interfaces,
                                            class_itables: self.class_itables,
                                        },
                                    )
                                {
                                    self.materialized_devirtualized_dispatch_target_fqn(
                                        e.span,
                                        &target_fqn,
                                    )
                                } else {
                                    self.dispatch_call_sites.insert(
                                        self.dispatch_call_site(e.span, receiver_ty),
                                        dispatch_kind,
                                    );
                                    fqn.clone()
                                }
                            } else {
                                self.dispatch_call_sites.insert(
                                    self.dispatch_call_site(e.span, receiver_ty),
                                    dispatch_kind,
                                );
                                fqn.clone()
                            }
                        } else {
                            self.materialized_direct_call_target_fqn_for_binding(&fun_binding)
                                .unwrap_or_else(|| fqn.clone())
                        };
                        let callee = self.top_level_callee_expr_with_fqn(callee.span, target_fqn);
                        let plan = self.callable_param_plan_for_fun_binding(&fun_binding);
                        if let Some((kind, ty)) =
                            self.lower_canonical_call_expr(CanonicalCallLoweringRequest {
                                pkg_prefix,
                                call_span: e.span,
                                callee,
                                source_args: args,
                                receiver: Some(receiver.clone()),
                                binding: arg_binding,
                                plan,
                                call_ty: member_call_ty,
                            })
                        {
                            return Some((kind, ty));
                        }
                    }

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    let mut positional_index = 0usize;
                    for arg in args {
                        let expected = self.expected_expr_for_fun_call_arg(
                            overload.as_ref(),
                            arg,
                            positional_index,
                        );
                        if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                            positional_index = positional_index.saturating_add(1);
                        }
                        lowered_args
                            .push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                    }

                    let receiver_ty = match lowered_args.first() {
                        Some(CallArg::Positional(receiver)) => receiver.ty,
                        Some(CallArg::Named { value, .. }) => value.ty,
                        None => self.builtins.any,
                    };
                    let dispatch_kind = if owner_is_interface {
                        Some(crate::hir::DispatchCallKind::Interface)
                    } else if owner_uses_virtual_dispatch {
                        Some(crate::hir::DispatchCallKind::Virtual)
                    } else {
                        None
                    };
                    let target_fqn = if let Some(dispatch_kind) = dispatch_kind {
                        if self.devirtualize_dispatch_calls {
                            if let Some(target_fqn) =
                                crate::devirtualize::try_devirtualize_dispatch_target(
                                    dispatch_kind,
                                    owner_fqn,
                                    member_name,
                                    args.len(),
                                    receiver_ty,
                                    self.types,
                                    crate::devirtualize::DispatchTargetFacts {
                                        known_receiver_subclasses: self.known_receiver_subclasses,
                                        class_vtables: self.class_vtables,
                                        interfaces: self.interfaces,
                                        class_itables: self.class_itables,
                                    },
                                )
                            {
                                self.materialized_devirtualized_dispatch_target_fqn(
                                    e.span,
                                    &target_fqn,
                                )
                            } else {
                                self.dispatch_call_sites.insert(
                                    self.dispatch_call_site(e.span, receiver_ty),
                                    dispatch_kind,
                                );
                                fqn.clone()
                            }
                        } else {
                            self.dispatch_call_sites.insert(
                                self.dispatch_call_site(e.span, receiver_ty),
                                dispatch_kind,
                            );
                            fqn.clone()
                        }
                    } else {
                        self.materialized_top_level_fun_call_target_fqn(e.span)
                            .unwrap_or_else(|| fqn.clone())
                    };
                    let callee = self.top_level_callee_expr_with_fqn(callee.span, target_fqn);

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        member_call_ty,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) = self.try_lower_struct_ctor_call_expr(
                    pkg_prefix,
                    e.span,
                    callee,
                    args,
                    typechecked_call_ty,
                ) {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else if let Some((kind, ty)) =
                    self.try_lower_default_args_call_expr(pkg_prefix, e.span, callee, args)
                {
                    (kind, typechecked_call_ty.unwrap_or(ty))
                } else if let Some((kind, ty)) = self.try_lower_retained_builtin_member_call_expr(
                    pkg_prefix, e.span, callee, args, call_ty,
                ) {
                    (kind, ty)
                } else {
                    // class ctor call 仍会被降低为 `UnresolvedIdent`，
                    // 但 codegen 需要知道 typecheck 已选中的 ctor 目标与参数绑定。
                    //
                    // P4-T01h：`Container<Int>(...)` 在 AST 中是 `Call(TypeApply(Ident, ...), ...)`，
                    // ctor 绑定本身仍键于 `e.span`，因此识别 callee 时需要把 `TypeApply` 透明展开。
                    if let ast::ExprKind::Ident(id) = &self.transparent_call_callee(callee).kind
                        && let Some(binding) = self
                            .typechecked_ctor_call_binding(e.span)
                            .or_else(|| self.resolver_fallback_ctor_call_binding(id, args))
                        && matches!(
                            self.type_kinds.get(&binding.owner_fqn),
                            Some(ast::TypeKind::Class)
                        )
                    {
                        self.ctor_call_sites
                            .entry(self.call_site(e.span))
                            .or_insert(crate::hir::CtorCallInfo {
                                class_fqn: binding.owner_fqn,
                                ctor_span: binding.ctor_span,
                                arg_mapping: binding.arg_mapping,
                            });
                    }

                    let callee_fqn = self.callee_top_level_fqn(callee);
                    let overload = callee_fqn.and_then(|fqn| self.fun_overload_by_fqn(fqn));

                    // T0113: find the vararg param index (if any) from the callee sig.
                    let vararg_param_index = overload.as_ref().and_then(|overload| {
                        let s = &overload.sig;
                        // Account for receiver: if the function has a receiver, params
                        // in the sig start with it, but call args don't include receiver.
                        let offset = if s.receiver.is_some() { 1 } else { 0 };
                        s.params.iter().enumerate().find_map(|(i, p)| {
                            if p.is_vararg {
                                Some(i.saturating_sub(offset))
                            } else {
                                None
                            }
                        })
                    });

                    let materialized_direct_target =
                        self.materialized_top_level_fun_call_target_fqn(e.span);
                    let callee = if let Some(target_fqn) = materialized_direct_target {
                        Box::new(self.top_level_callee_expr_with_fqn(callee.span, target_fqn))
                    } else {
                        Box::new(self.lower_expr(pkg_prefix, callee))
                    };

                    let arg_binding = self.typechecked_call_arg_binding(e.span);
                    let preserve_named_call_args = arg_binding.is_some();
                    if let Some(arg_binding) = arg_binding {
                        let ctor_binding = self.typechecked_ctor_call_binding(e.span);
                        let canonical_param_count = arg_binding.params.len();
                        let receiver = self
                            .call_arg_binding_has_receiver(&arg_binding)
                            .then(|| {
                                self.lower_canonical_receiver_from_member_callee(
                                    pkg_prefix,
                                    callee_expr,
                                )
                            })
                            .flatten();
                        let plan = if let Some(fun_binding) =
                            self.typechecked_top_level_fun_call_binding(e.span)
                        {
                            self.callable_param_plan_for_fun_binding(&fun_binding)
                        } else if let Some(ctor_binding) = ctor_binding.as_ref() {
                            self.callable_param_plan_for_ctor_binding(e.span, ctor_binding)
                        } else {
                            None
                        };
                        if let Some((kind, ty)) =
                            self.lower_canonical_call_expr(CanonicalCallLoweringRequest {
                                pkg_prefix,
                                call_span: e.span,
                                callee: callee.as_ref().clone(),
                                source_args: args,
                                receiver,
                                binding: arg_binding,
                                plan,
                                call_ty,
                            })
                        {
                            if ctor_binding.is_some()
                                && let Some(site) =
                                    self.ctor_call_sites.get_mut(&self.call_site(e.span))
                            {
                                site.arg_mapping = (0..canonical_param_count).map(Some).collect();
                            }
                            return Expr {
                                span: e.span,
                                ty,
                                kind,
                            };
                        }
                    }

                    // T0113: if there's a vararg param, split args into pre-vararg,
                    // vararg, and post-vararg, and wrap the vararg args in an array literal.
                    let lowered_args = if let Some(va_idx) = vararg_param_index {
                        self.lower_call_args_with_vararg(
                            pkg_prefix,
                            e.span,
                            args,
                            overload.as_ref(),
                            va_idx,
                        )
                    } else {
                        let mut positional_index = 0usize;
                        let mut out: Vec<CallArg> = Vec::with_capacity(args.len());
                        for arg in args {
                            let expected = self.expected_expr_for_fun_call_arg(
                                overload.as_ref(),
                                arg,
                                positional_index,
                            );
                            if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                                positional_index = positional_index.saturating_add(1);
                            }
                            out.push(if preserve_named_call_args {
                                self.lower_call_arg_with_expected_preserving_name(
                                    pkg_prefix, arg, expected,
                                )
                            } else {
                                self.lower_call_arg_with_expected(pkg_prefix, arg, expected)
                            });
                        }
                        out
                    };

                    (
                        ExprKind::Call {
                            callee,
                            args: lowered_args,
                        },
                        call_ty,
                    )
                }
            }
            // Parser/typecheck gate 确保 named/spread 只出现在调用实参语境；若恢复路径仍把
            // 语法糖节点递给普通表达式 lowering，这里剥掉语法壳而不是产出 HIR placeholder。
            ast::ExprKind::SpreadArg { expr, .. } => {
                let inner = self.lower_expr_with_expected(pkg_prefix, expr, expected);
                (inner.kind, inner.ty)
            }
            ast::ExprKind::NamedArg { value, .. } => {
                let inner = self.lower_expr_with_expected(pkg_prefix, value, expected);
                (inner.kind, inner.ty)
            }
            ast::ExprKind::TupleLit { elements } => {
                let elements: Vec<Expr> = elements
                    .iter()
                    .map(|e| self.lower_expr(pkg_prefix, e))
                    .collect();
                let inferred_ty = if elements.is_empty() {
                    self.builtins.unit
                } else {
                    self.types.ty_tuple(elements.iter().map(|e| e.ty).collect())
                };
                let ty = self.typechecked_expr_ty(e.span).unwrap_or(inferred_ty);
                (ExprKind::TupleLit { elements }, ty)
            }
            ast::ExprKind::Lambda(lam) => self.lower_lambda_expr(pkg_prefix, e.span, lam),
            ast::ExprKind::StructLit { ty, fields } => {
                self.lower_struct_lit_expr(pkg_prefix, e.span, ty, fields, expected.struct_lit_ty)
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = Box::new(self.lower_expr(pkg_prefix, cond));
                let then_branch =
                    Box::new(self.lower_expr_with_expected(pkg_prefix, then_branch, expected));
                let else_branch = else_branch
                    .as_ref()
                    .map(|e| Box::new(self.lower_expr_with_expected(pkg_prefix, e, expected)));
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .or(expected.value_ty)
                    .or(expected.array_lit_ty)
                    .or(expected.struct_lit_ty)
                    .unwrap_or(self.builtins.any);
                (
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    ty,
                )
            }
            ast::ExprKind::When { subject, arms } => {
                let subject = Box::new(self.lower_expr(pkg_prefix, subject));
                let arms = arms
                    .iter()
                    .map(|a| self.lower_when_arm(pkg_prefix, a, expected))
                    .collect();
                let ty = self
                    .typechecked_expr_ty(e.span)
                    .or(expected.value_ty)
                    .or(expected.array_lit_ty)
                    .or(expected.struct_lit_ty)
                    .unwrap_or(self.builtins.any);
                (ExprKind::When { subject, arms }, ty)
            }
            ast::ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                let handle = self.lower_handle_expr(pkg_prefix, body, arms, finally.as_ref());
                let ty = self.typechecked_expr_ty(e.span).unwrap_or(handle.body.ty);
                (ExprKind::Handle(handle), ty)
            }
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(pkg_prefix, e.span, receiver, member)
            }
            ast::ExprKind::SpliceField { receiver, field } => {
                self.lower_splice_field_expr(pkg_prefix, e.span, receiver, field)
            }
            ast::ExprKind::SafeMemberAccess {
                receiver,
                op_span,
                member,
            } => self.lower_safe_member_access_expr(pkg_prefix, e.span, receiver, *op_span, member),
            ast::ExprKind::NotNullAssert { expr, op_span } => {
                self.lower_not_null_assert_expr(pkg_prefix, e.span, expr, *op_span)
            }
            ast::ExprKind::Unary { op, op_span, expr } => {
                if let Some((kind, ty)) = self
                    .try_lower_typechecked_operator_overload_unary_expr(pkg_prefix, e.span, expr)
                {
                    (kind, ty)
                } else if let Some(call) =
                    self.try_lower_builtin_unary_operator_expr(pkg_prefix, e.span, *op, expr)
                {
                    (call.kind, call.ty)
                } else {
                    let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                    let heuristic_ty = match op {
                        ast::UnaryOp::Not => {
                            if expr.ty == self.builtins.bool_ {
                                self.builtins.bool_
                            } else {
                                self.builtins.any
                            }
                        }
                        ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                            if self.is_integer_type(expr.ty) {
                                expr.ty
                            } else {
                                self.builtins.any
                            }
                        }
                    };
                    let ty = self.typechecked_expr_ty(e.span).unwrap_or(heuristic_ty);
                    (
                        ExprKind::Unary {
                            op: *op,
                            op_span: *op_span,
                            expr,
                        },
                        ty,
                    )
                }
            }
            ast::ExprKind::Binary {
                lhs,
                op,
                op_span,
                rhs,
            } => {
                if *op == ast::BinaryOp::RangeInclusive {
                    return self.lower_range_inclusive_expr(pkg_prefix, e.span, *op_span, lhs, rhs);
                }
                if *op == ast::BinaryOp::Elvis {
                    return self.lower_elvis_expr(pkg_prefix, e.span, lhs, *op_span, rhs);
                }
                if matches!(
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
                ) && let Some((kind, ty)) = self
                    .try_lower_typechecked_operator_overload_binary_expr(
                        pkg_prefix, e.span, lhs, rhs,
                    )
                {
                    (kind, ty)
                } else if matches!(
                    op,
                    ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge
                ) && let Some((kind, ty)) = self
                    .try_lower_typechecked_compare_to_binary_expr(
                        pkg_prefix, e.span, *op, *op_span, lhs, rhs,
                    )
                {
                    (kind, ty)
                } else if let Some(call) =
                    self.try_lower_builtin_binary_operator_expr(pkg_prefix, e.span, *op, lhs, rhs)
                {
                    (call.kind, call.ty)
                } else {
                    let lhs = Box::new(self.lower_expr(pkg_prefix, lhs));
                    let rhs = Box::new(self.lower_expr(pkg_prefix, rhs));
                    let ty = self
                        .typechecked_expr_ty(e.span)
                        .unwrap_or_else(|| self.lower_binary_expr_type(&lhs, &rhs, *op));
                    (
                        ExprKind::Binary {
                            lhs,
                            op: *op,
                            op_span: *op_span,
                            rhs,
                        },
                        ty,
                    )
                }
            }
            ast::ExprKind::Assign { .. } => {
                self.record_stage_error(
                    e.span,
                    "assignment expression cannot enter HIR lowering",
                    "HIR expression lowering",
                );
                self.invalid_expr_kind_after_stage_error(e.span)
            }
            ast::ExprKind::TypeCheck {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                (
                    ExprKind::TypeCheck {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    self.builtins.bool_,
                )
            }
            ast::ExprKind::Cast {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                let out_ty = match op {
                    ast::CastOp::As => target_ty,
                    ast::CastOp::AsQ => self.types.ty_option(target_ty),
                };
                (
                    ExprKind::Cast {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    out_ty,
                )
            }
            ast::ExprKind::WithUpdate {
                base,
                with_span,
                updates,
            } => {
                return self.lower_with_update_expr(pkg_prefix, e.span, *with_span, base, updates);
            }
        };

        Expr {
            span: e.span,
            ty,
            kind,
        }
    }

    pub(in crate::hir::lower) fn desugar_f_string_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        raw: bool,
        parts: &[ast::InterpolatedStringPart],
    ) -> Expr {
        let builder_ty =
            self.intern_nominal(Self::STRING_BUILDER_FQN.to_string(), Vec::new(), None);
        let (builder_decl_span, builder_id, builder_name) =
            self.fresh_synthetic_local(span, "__sb", false);

        let ctor_call_span = self.fresh_synthetic_call_site_span(span);
        let ctor_span = self
            .index
            .constructors
            .get(Self::STRING_BUILDER_FQN)
            .and_then(|ctors| {
                ctors
                    .iter()
                    .find(|ctor| ctor.params.is_empty())
                    .map(|ctor| ctor.span)
            });
        if ctor_span.is_none() {
            self.record_stage_error(
                span,
                "StringBuilder constructor missing for f-string desugar",
                "HIR f-string desugar",
            );
        }
        self.ctor_call_sites.insert(
            self.call_site(ctor_call_span),
            crate::hir::CtorCallInfo {
                class_fqn: Self::STRING_BUILDER_FQN.to_string(),
                ctor_span,
                arg_mapping: Vec::new(),
            },
        );

        let ctor_call = Expr {
            span: ctor_call_span,
            ty: builder_ty,
            kind: ExprKind::Call {
                callee: Box::new(Expr {
                    span: ctor_call_span,
                    ty: self.builtins.any,
                    kind: ExprKind::UnresolvedIdent {
                        name: "StringBuilder".to_string(),
                    },
                }),
                args: Vec::new(),
            },
        };

        let mut stmts = vec![Stmt {
            span: builder_decl_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: builder_decl_span,
                id: Some(builder_id),
                name: Some(builder_name.clone()),
                mutable: false,
                ty: builder_ty,
                init: Some(ctor_call),
            }),
        }];

        for part in parts {
            match part {
                ast::InterpolatedStringPart::Text { span: text_span } => {
                    let decoded = match parse_f_string_text_utf8(raw, self.source.slice(*text_span))
                    {
                        Ok(decoded) => decoded,
                        Err(_) => {
                            self.record_stage_error(
                                *text_span,
                                "invalid f-string text segment",
                                "HIR f-string desugar",
                            );
                            String::new()
                        }
                    };
                    if decoded.is_empty() {
                        continue;
                    }
                    let text_expr = Expr {
                        span: *text_span,
                        ty: self.builtins.string,
                        kind: ExprKind::Literal(LiteralKind::SynthString(decoded)),
                    };
                    let add_call_span = self.fresh_synthetic_call_site_span(*text_span);
                    stmts.push(self.string_builder_add_stmt(
                        add_call_span,
                        builder_decl_span,
                        builder_id,
                        &builder_name,
                        builder_ty,
                        text_expr,
                    ));
                }
                ast::InterpolatedStringPart::Expr { expr } => {
                    let lowered_expr = self.lower_expr(pkg_prefix, expr);
                    let to_string_call_span = self.fresh_synthetic_call_site_span(expr.span);
                    let to_string_call = self.lower_synthetic_member_call(
                        to_string_call_span,
                        lowered_expr,
                        Self::TO_STRING_INTERFACE_METHOD_FQN,
                        Vec::new(),
                        self.builtins.string,
                    );
                    let add_call_span = self.fresh_synthetic_call_site_span(expr.span);
                    stmts.push(self.string_builder_add_stmt(
                        add_call_span,
                        builder_decl_span,
                        builder_id,
                        &builder_name,
                        builder_ty,
                        to_string_call,
                    ));
                }
            }
        }

        let finish_call_span = self.fresh_synthetic_call_site_span(span);
        let finish_call = self.lower_synthetic_member_call(
            finish_call_span,
            self.string_builder_ref_expr(builder_decl_span, builder_id, &builder_name, builder_ty),
            Self::STRING_BUILDER_TO_STRING_FQN,
            Vec::new(),
            self.builtins.string,
        );
        stmts.push(Stmt {
            span: finish_call_span,
            ty: self.builtins.string,
            kind: StmtKind::Expr(finish_call),
        });

        Expr {
            span,
            ty: self.builtins.string,
            kind: ExprKind::Block(Block {
                span,
                ty: self.builtins.string,
                stmts,
            }),
        }
    }

    fn string_builder_ref_expr(
        &self,
        decl_span: Span,
        id: crate::hir::SymbolId,
        name: &str,
        ty: TypeId,
    ) -> Expr {
        Expr {
            span: decl_span,
            ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id,
                name: name.to_string(),
                decl_span,
            }),
        }
    }

    fn string_builder_add_stmt(
        &mut self,
        call_span: Span,
        builder_decl_span: Span,
        builder_id: crate::hir::SymbolId,
        builder_name: &str,
        builder_ty: TypeId,
        value: Expr,
    ) -> Stmt {
        let add_call = self.lower_synthetic_member_call(
            call_span,
            self.string_builder_ref_expr(builder_decl_span, builder_id, builder_name, builder_ty),
            Self::STRING_BUILDER_ADD_FQN,
            vec![value],
            builder_ty,
        );
        Stmt {
            span: call_span,
            ty: builder_ty,
            kind: StmtKind::Expr(add_call),
        }
    }

    /// `lhs .. rhs` → `{ val __range_start = lhs; val __range_end = rhs; rangeTo(__range_start, __range_end, __scoop_range_default_step(__range_start)) }`
    ///
    /// 说明：
    /// - 复用现有 `scoop.core.rangeTo(start, endInclusive, step)` 实现，不在后端新增 special-case；
    /// - 显式引入临时变量，保证左右端点只求值一次；
    /// - `step = 1` 通过 stdlib helper `__scoop_range_default_step` 派生，避免在 lowering 中伪造源码字面量。
    pub(in crate::hir::lower) fn lower_range_inclusive_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        op_span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Expr {
        let progression_ty = self.typechecked_expr_ty(span).unwrap_or_else(|| {
            self.intern_nominal(Self::INT_PROGRESSION_FQN.to_string(), Vec::new(), None)
        });

        let start_expr = self.lower_expr(pkg_prefix, lhs);
        let end_expr = self.lower_expr(pkg_prefix, rhs);

        let start_decl_span = Span::new(op_span.start, op_span.start + 1);
        let end_decl_span = Span::new(op_span.start + 1, op_span.start + 2);

        let start_id = self.intern_local_symbol(start_decl_span, false);
        let end_id = self.intern_local_symbol(end_decl_span, false);
        let start_name = "__range_start".to_string();
        let end_name = "__range_end".to_string();

        let start_ty = start_expr.ty;
        let end_ty = end_expr.ty;

        let start_decl = Stmt {
            span: start_decl_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: start_decl_span,
                id: Some(start_id),
                name: Some(start_name.clone()),
                mutable: false,
                ty: start_ty,
                init: Some(start_expr),
            }),
        };

        let end_decl = Stmt {
            span: end_decl_span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(ValDecl {
                span: end_decl_span,
                id: Some(end_id),
                name: Some(end_name.clone()),
                mutable: false,
                ty: end_ty,
                init: Some(end_expr),
            }),
        };

        let start_ref = Expr {
            span: start_decl_span,
            ty: start_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: start_id,
                name: start_name.clone(),
                decl_span: start_decl_span,
            }),
        };
        let end_ref = Expr {
            span: end_decl_span,
            ty: end_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: end_id,
                name: end_name.clone(),
                decl_span: end_decl_span,
            }),
        };

        let step_helper = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self
                    .symbols
                    .intern_top_level(Self::RANGE_DEFAULT_STEP_FQN.to_string()),
                fqn: Self::RANGE_DEFAULT_STEP_FQN.to_string(),
            }),
        };
        let step_expr = Expr {
            span: op_span,
            ty: self.builtins.int,
            kind: ExprKind::Call {
                callee: Box::new(step_helper),
                args: vec![CallArg::Positional(start_ref.clone())],
            },
        };

        let range_to_callee = Expr {
            span: op_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self
                    .symbols
                    .intern_top_level(Self::RANGE_TO_FQN.to_string()),
                fqn: Self::RANGE_TO_FQN.to_string(),
            }),
        };
        let range_call = Expr {
            span,
            ty: progression_ty,
            kind: ExprKind::Call {
                callee: Box::new(range_to_callee),
                args: vec![
                    CallArg::Positional(start_ref),
                    CallArg::Positional(end_ref),
                    CallArg::Positional(step_expr),
                ],
            },
        };

        Expr {
            span,
            ty: progression_ty,
            kind: ExprKind::Block(Block {
                span,
                ty: progression_ty,
                stmts: vec![
                    start_decl,
                    end_decl,
                    Stmt {
                        span,
                        ty: progression_ty,
                        kind: StmtKind::Expr(range_call),
                    },
                ],
            }),
        }
    }

    /// 从一个 `TypeRef` 判定数组字面量的目标容器类型（Array vs MutableArray）。
    pub(in crate::hir::lower) fn array_lit_target_from_type_ref(
        &self,
        ty: &ast::TypeRef,
    ) -> Option<ArrayLitTarget> {
        // 注意：`TypeRef` 可能来自其它源文件（例如通过 `Index::FunSig` 跨文件查询得到的签名）。
        // 当前 HIR lowering 仍以“单个 SourceFile 负责 span → 文本切片”为前提，因此当 span 不在
        // 当前文件范围内时，我们只能保守放弃该 hint，避免越界 panic。
        let span = ty.span();
        let text = self.source.text();
        if span.end > text.len() {
            return None;
        }
        // UTF-8 防线：跨文件 span（或内部 bug）可能导致 start/end 落在非字符边界上，
        // 直接 slice 会 panic。这里同样保守放弃 hint。
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return None;
        }

        let fqn = self
            .index
            .type_ref_to_fqn_in_file(self.source, self.file, ty)?;
        match fqn.as_str() {
            "scoop.core.Array" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableArray" => Some(ArrayLitTarget::MutableArray),
            // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
            // lowering 阶段只需要知道“数组字面量目标容器类型”，因此这里把别名也视为等价目标。
            "scoop.core.List" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableList" => Some(ArrayLitTarget::MutableArray),
            // T1317f4：stdlib `Set/MutableSet/MapView/MutableMap` 当前阶段以数组为底座（typealias）。
            // 这里同样把它们视为 array literal 的等价目标，便于写 `val s: MutableSet = []` 等用例。
            "scoop.collections.Set" => Some(ArrayLitTarget::Array),
            "scoop.collections.MapView" => Some(ArrayLitTarget::Array),
            "scoop.collections.MutableSet" => Some(ArrayLitTarget::MutableArray),
            "scoop.collections.MutableMap" => Some(ArrayLitTarget::MutableArray),
            _ => None,
        }
    }

    /// 尝试把“当前文件内”的 `TypeRef` 直接 lower 为 `TypeId`。
    ///
    /// 说明：
    /// - `Index::FunSig` 里的 `TypeRef` 可能来自别的源文件；
    /// - HIR lowering 仍以当前 `SourceFile` 负责 span 切片为前提，因此这里只在
    ///   span 明确落在当前文件且满足 UTF-8 边界时才做该回退；
    /// - 失败时返回 `None`，让调用方继续走更保守的 fallback。
    pub(in crate::hir::lower) fn local_type_ref_ty(&mut self, ty: &ast::TypeRef) -> Option<TypeId> {
        let span = ty.span();
        let text = self.source.text();
        if span.start > text.len() || span.end > text.len() {
            return None;
        }
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return None;
        }
        Some(self.lower_type_ref(ty))
    }

    /// 在声明源文件上下文里把 `TypeRef` 解析为 `TypeId`。
    ///
    /// 说明：
    /// - imported/sysroot `FunSig` 保留的是“声明处 AST”，其 `TypeRef` span 只对声明源文件有效；
    /// - 若在 caller 文件里直接切这些 span，轻则误解析，重则在 UTF-8 注释上命中非字符边界并 panic；
    /// - 因此 expected-type hint 需要先切回声明源文件上下文，再复用本地 lowering 逻辑。
    pub(in crate::hir::lower) fn type_ref_ty_in_decl_context(
        &mut self,
        decl_file: &std::path::Path,
        ty: &ast::TypeRef,
    ) -> Option<TypeId> {
        if decl_file == self.source.path() {
            return self.local_type_ref_ty(ty);
        }
        let (decl_source, decl_ast) = self.decl_ast_context(decl_file)?;
        self.with_foreign_ast_context(decl_source, decl_ast, |this| this.local_type_ref_ty(ty))
    }

    pub(in crate::hir::lower) fn array_lit_target_from_type_ref_in_decl_context(
        &mut self,
        decl_file: &std::path::Path,
        ty: &ast::TypeRef,
    ) -> Option<ArrayLitTarget> {
        if decl_file == self.source.path() {
            return self.array_lit_target_from_type_ref(ty);
        }
        let (decl_source, decl_ast) = self.decl_ast_context(decl_file)?;
        self.with_foreign_ast_context(decl_source, decl_ast, |this| {
            this.array_lit_target_from_type_ref(ty)
        })
    }

    pub(in crate::hir::lower) fn typechecked_expr_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_expr_ty(span)?;
        let ty = self.types.re_intern_from(typecheck_types, ty);
        Some(self.apply_active_type_param_bindings(ty))
    }

    pub(in crate::hir::lower) fn typechecked_splice_field_contract(
        &mut self,
        span: Span,
    ) -> Option<LoweredSpliceFieldContract> {
        let typecheck_types = self.typecheck_types?;
        let contract = self.file.splice_field_contract(span)?;
        let field_ty = self
            .types
            .re_intern_from(typecheck_types, contract.field_ty);
        Some(LoweredSpliceFieldContract {
            field_name: contract.field_name,
            field_fqn: contract.field_fqn,
            field_ty: self.apply_active_type_param_bindings(field_ty),
        })
    }

    pub(in crate::hir::lower) fn typechecked_with_update_contract(
        &mut self,
        span: Span,
    ) -> Option<ast::WithUpdateContract> {
        let typecheck_types = self.typecheck_types?;
        let contract = self.file.with_update_contract(span)?;
        Some(Self::re_intern_with_update_contract_types(
            self.types,
            typecheck_types,
            contract,
        ))
    }

    pub(in crate::hir::lower) fn re_intern_with_update_contract_types(
        types: &mut TypeStore,
        typecheck_types: &TypeStore,
        contract: ast::WithUpdateContract,
    ) -> ast::WithUpdateContract {
        let re_ty = |types: &mut TypeStore, ty| types.re_intern_from(typecheck_types, ty);
        ast::WithUpdateContract {
            base_ty: re_ty(types, contract.base_ty),
            result_ty: re_ty(types, contract.result_ty),
            aggregates: contract
                .aggregates
                .into_iter()
                .map(|aggregate| ast::WithUpdateAggregateContract {
                    prefix: aggregate.prefix,
                    ty: re_ty(types, aggregate.ty),
                    kind: match aggregate.kind {
                        ast::WithUpdateAggregateContractKind::Struct { fqn, fields } => {
                            ast::WithUpdateAggregateContractKind::Struct {
                                fqn,
                                fields: fields
                                    .into_iter()
                                    .map(|field| ast::WithUpdateAggregateFieldContract {
                                        name: field.name,
                                        ty: re_ty(types, field.ty),
                                    })
                                    .collect(),
                            }
                        }
                        ast::WithUpdateAggregateContractKind::Tuple { elements } => {
                            ast::WithUpdateAggregateContractKind::Tuple {
                                elements: elements.into_iter().map(|ty| re_ty(types, ty)).collect(),
                            }
                        }
                        ast::WithUpdateAggregateContractKind::Enum { info } => {
                            ast::WithUpdateAggregateContractKind::Enum {
                                info: ast::WithUpdateResolvedEnum {
                                    enum_fqn: info.enum_fqn,
                                    variants: info
                                        .variants
                                        .into_iter()
                                        .map(|variant| ast::WithUpdateResolvedEnumVariant {
                                            name: variant.name,
                                            fields: variant
                                                .fields
                                                .into_iter()
                                                .map(|field| ast::WithUpdateResolvedEnumField {
                                                    name: field.name,
                                                    ty: re_ty(types, field.ty),
                                                })
                                                .collect(),
                                        })
                                        .collect(),
                                },
                            }
                        }
                    },
                })
                .collect(),
            updates: contract
                .updates
                .into_iter()
                .map(|update| ast::WithUpdateUpdateContract {
                    path: update.path,
                    target_ty: re_ty(types, update.target_ty),
                    value_ty: re_ty(types, update.value_ty),
                    segments: update
                        .segments
                        .into_iter()
                        .map(|segment| ast::WithUpdatePathSegmentContract {
                            aggregate_prefix: segment.aggregate_prefix,
                            aggregate_ty: re_ty(types, segment.aggregate_ty),
                            field_ty: re_ty(types, segment.field_ty),
                            kind: segment.kind,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub(in crate::hir::lower) fn missing_with_update_expr(&mut self, span: Span) -> Expr {
        self.record_stage_error(
            span,
            "with-update expression missing typed aggregate contract",
            "HIR expression lowering",
        );
        self.invalid_expr_after_stage_error(span)
    }

    pub(in crate::hir::lower) fn typechecked_binding_ty(&mut self, span: Span) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_binding_ty(span)?;
        let ty = self.types.re_intern_from(typecheck_types, ty);
        Some(self.apply_active_type_param_bindings(ty))
    }

    pub(in crate::hir::lower) fn typechecked_fun_return_ty(
        &mut self,
        span: Span,
    ) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_fun_return_ty(span)?;
        let ty = self.types.re_intern_from(typecheck_types, ty);
        Some(self.apply_active_type_param_bindings(ty))
    }

    pub(in crate::hir::lower) fn option_inner_ty(&self, ty: TypeId) -> Option<TypeId> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => Some(*inner),
            _ => None,
        }
    }

    pub(in crate::hir::lower) fn typechecked_performed_effect_ty(
        &mut self,
        span: Span,
    ) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_performed_effect_ty(span)?;
        let ty = self.types.re_intern_from(typecheck_types, ty);
        Some(self.apply_active_type_param_bindings(ty))
    }

    pub(in crate::hir::lower) fn typechecked_handle_arm_effect_ty(
        &mut self,
        span: Span,
    ) -> Option<TypeId> {
        let typecheck_types = self.typecheck_types?;
        let ty = self.file.inferred_handle_arm_effect_ty(span)?;
        let ty = self.types.re_intern_from(typecheck_types, ty);
        Some(self.apply_active_type_param_bindings(ty))
    }

    pub(in crate::hir::lower) fn zero_arg_unit_call_uses_sugar(&self, span: Span) -> bool {
        self.file.zero_arg_unit_call_uses_sugar(span)
    }

    pub(in crate::hir::lower) fn synthesized_unit_call_args_for_typed_sugar(
        &self,
        call_span: Span,
        args: &[ast::Expr],
    ) -> Option<Vec<ast::Expr>> {
        self.zero_arg_unit_call_uses_sugar(call_span).then(|| {
            debug_assert!(args.is_empty(), "typed Unit sugar 只应标记原始零参调用点");
            vec![ast::Expr {
                span: call_span,
                kind: ast::ExprKind::UnitLit,
            }]
        })
    }

    pub(in crate::hir::lower) fn typechecked_effect_op_call_binding(
        &mut self,
        span: Span,
    ) -> Option<crate::ast::EffectOpCallBinding> {
        let typecheck_types = self.typecheck_types?;
        let binding = self.file.typechecked_effect_op_call_binding(span)?;
        let op_type_args = binding
            .op_type_args
            .into_iter()
            .map(|ty| {
                let ty = self.types.re_intern_from(typecheck_types, ty);
                self.apply_active_type_param_bindings(ty)
            })
            .collect();
        Some(crate::ast::EffectOpCallBinding {
            arg_mapping: binding.arg_mapping,
            op_type_args,
        })
    }

    pub(in crate::hir::lower) fn typechecked_handle_arm_op_type_args(
        &mut self,
        span: Span,
    ) -> Option<Vec<TypeId>> {
        let typecheck_types = self.typecheck_types?;
        let args = self.file.inferred_handle_arm_op_type_args(span)?;
        Some(
            args.into_iter()
                .map(|ty| {
                    let ty = self.types.re_intern_from(typecheck_types, ty);
                    self.apply_active_type_param_bindings(ty)
                })
                .collect(),
        )
    }

    pub(in crate::hir::lower) fn typechecked_top_level_fun_value_ref(
        &mut self,
        span: Span,
    ) -> Option<crate::ast::TopLevelFunValueRef> {
        let typecheck_types = self.typecheck_types?;
        let fun_ref = self.file.top_level_fun_value_ref(span)?;
        let type_args = fun_ref
            .type_args
            .iter()
            .copied()
            .map(|ty| {
                let ty = self.types.re_intern_from(typecheck_types, ty);
                self.apply_active_type_param_bindings(ty)
            })
            .collect();
        let eff_args = fun_ref
            .eff_args
            .iter()
            .map(|row| self.apply_active_type_param_bindings_to_effect_row(row))
            .collect();
        Some(crate::ast::TopLevelFunValueRef {
            fqn: fun_ref.fqn,
            decl_file: fun_ref.decl_file,
            decl_span: fun_ref.decl_span,
            type_args,
            eff_args,
        })
    }

    pub(in crate::hir::lower) fn typechecked_top_level_fun_call_binding(
        &mut self,
        span: Span,
    ) -> Option<crate::ast::TopLevelFunCallBinding> {
        let typecheck_types = self.typecheck_types?;
        let binding = self.file.top_level_fun_call_binding(span)?;
        let type_args = binding
            .type_args
            .iter()
            .copied()
            .map(|ty| {
                let ty = self.types.re_intern_from(typecheck_types, ty);
                self.apply_active_type_param_bindings(ty)
            })
            .collect();
        let eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                let re_interned = EffectRow::new(
                    row.terms
                        .iter()
                        .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                        .collect(),
                );
                self.apply_active_type_param_bindings_to_effect_row(&re_interned)
            })
            .collect();
        Some(crate::ast::TopLevelFunCallBinding {
            fqn: binding.fqn,
            decl_file: binding.decl_file,
            decl_span: binding.decl_span,
            is_intrinsic: binding.is_intrinsic,
            intrinsic_entry_name: binding.intrinsic_entry_name,
            type_args,
            eff_args,
        })
    }

    pub(in crate::hir::lower) fn typechecked_call_arg_binding(
        &self,
        span: Span,
    ) -> Option<crate::ast::CallArgBinding> {
        self.file.typechecked_call_arg_binding(span)
    }

    pub(in crate::hir::lower) fn callable_param_plan_for_fun_binding(
        &mut self,
        binding: &crate::ast::TopLevelFunCallBinding,
    ) -> Option<CallableParamPlan> {
        let (decl_source, decl_file) = self.decl_ast_context(&binding.decl_file)?;
        let (fun, type_param_names) =
            find_fun_decl_with_type_params(decl_source, decl_file, binding.decl_span)?;
        let type_param_bindings = type_param_names
            .into_iter()
            .zip(binding.type_args.iter().copied())
            .collect();
        Some(CallableParamPlan {
            decl_file: binding.decl_file.clone(),
            type_param_bindings,
            params: param_infos_from_ast_params(decl_source, &fun.params),
        })
    }

    pub(in crate::hir::lower) fn callable_param_plan_for_ctor_binding(
        &mut self,
        call_span: Span,
        binding: &crate::ast::CtorCallBinding,
    ) -> Option<CallableParamPlan> {
        let ctor_span = binding.ctor_span?;
        let ctor = self
            .index
            .constructors
            .get(&binding.owner_fqn)?
            .iter()
            .find(|ctor| ctor.span == ctor_span)?;
        let (decl_source, decl_file) = self.decl_ast_context(&ctor.decl_file)?;
        let (params, type_param_names) =
            find_ctor_params_with_type_params(decl_source, decl_file, ctor_span)?;
        let type_args = self
            .typechecked_expr_ty(call_span)
            .and_then(|ty| match self.types.kind(ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                    if nominal.fqn == binding.owner_fqn =>
                {
                    Some(nominal.args.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        let type_param_bindings = type_param_names.into_iter().zip(type_args).collect();
        Some(CallableParamPlan {
            decl_file: ctor.decl_file.clone(),
            type_param_bindings,
            params,
        })
    }

    pub(in crate::hir::lower) fn materialized_instance_fqn_for_decl(
        &self,
        fqn: &str,
        decl_file: &std::path::Path,
        decl_span: Span,
        type_args: &[TypeId],
        eff_args: &[crate::ty::EffectRow],
    ) -> String {
        let template = crate::mir::TemplateKey {
            fqn: fqn.to_string(),
            source_path: decl_file.to_path_buf(),
            decl_span,
        };
        let symbol_suffix = self
            .generic_template_symbol_suffixes
            .get(&template)
            .map(String::as_str)
            .or_else(|| {
                self.generic_template_symbol_suffixes
                    .iter()
                    .find_map(|(candidate, suffix)| {
                        (candidate.fqn == fqn
                            && candidate.source_path.as_path() == decl_file
                            && candidate.decl_span.start <= decl_span.start
                            && decl_span.end <= candidate.decl_span.end)
                            .then_some(suffix.as_str())
                    })
            })
            .unwrap_or("");
        stable_instance_fqn(self.types, &template, type_args, eff_args, symbol_suffix)
    }
}
