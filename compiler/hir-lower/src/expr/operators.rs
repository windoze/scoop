use super::*;

mod short_circuit;

mod equality;

impl Lowerer {
    pub(crate) fn lower_component_call(
        &mut self,
        receiver: hir::Expr,
        index: std::num::NonZeroU32,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.lower_named_call_on_receiver(
            receiver,
            &ast::Ident {
                // The text is diagnostic metadata only. Candidate collection
                // is keyed by the typed role carried in `required`.
                text: format!("component{index}"),
                span,
            },
            CallSite {
                type_args: &[],
                args: &[],
                span,
            },
            sink,
            None,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Component { index }),
                infix: false,
                ..Default::default()
            },
        )
    }

    pub(super) fn lower_conventional_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let (kind, name, receiver, argument, negate, comparison) = match op {
            ast::BinOp::Add => (hir::OperatorKind::Plus, "plus", lhs, rhs, false, None),
            ast::BinOp::Sub => (hir::OperatorKind::Minus, "minus", lhs, rhs, false, None),
            ast::BinOp::Mul => (hir::OperatorKind::Times, "times", lhs, rhs, false, None),
            ast::BinOp::Div => (hir::OperatorKind::Div, "div", lhs, rhs, false, None),
            ast::BinOp::Rem => (hir::OperatorKind::Rem, "rem", lhs, rhs, false, None),
            ast::BinOp::RangeTo => (hir::OperatorKind::RangeTo, "rangeTo", lhs, rhs, false, None),
            ast::BinOp::RangeUntil => (
                hir::OperatorKind::RangeUntil,
                "rangeUntil",
                lhs,
                rhs,
                false,
                None,
            ),
            ast::BinOp::Contains => (
                hir::OperatorKind::Contains,
                "contains",
                rhs,
                lhs,
                false,
                None,
            ),
            ast::BinOp::NotContains => (
                hir::OperatorKind::Contains,
                "contains",
                rhs,
                lhs,
                true,
                None,
            ),
            ast::BinOp::Lt => (
                hir::OperatorKind::CompareTo,
                "compareTo",
                lhs,
                rhs,
                false,
                Some(hir::BinOp::Lt),
            ),
            ast::BinOp::Le => (
                hir::OperatorKind::CompareTo,
                "compareTo",
                lhs,
                rhs,
                false,
                Some(hir::BinOp::Le),
            ),
            ast::BinOp::Gt => (
                hir::OperatorKind::CompareTo,
                "compareTo",
                lhs,
                rhs,
                false,
                Some(hir::BinOp::Gt),
            ),
            ast::BinOp::Ge => (
                hir::OperatorKind::CompareTo,
                "compareTo",
                lhs,
                rhs,
                false,
                Some(hir::BinOp::Ge),
            ),
            _ => unreachable!("only conventional binary operators enter this path"),
        };
        let name = ast::Ident {
            text: name.to_string(),
            span,
        };
        let args = [ast::CallArgument::positional(argument.clone())];
        let call = CallSite {
            type_args: &[],
            args: &args,
            span,
        };
        let required = RequiredCallableModifiers {
            operator: Some(kind),
            infix: false,
            ..Default::default()
        };
        let value = if let Some(layer) = self.probe_numeric_literal_receiver(
            receiver,
            expected,
            |state, receiver, layer_sink| {
                if let Some(comparison) = comparison
                    && let Some(kind) = state.float_kind(receiver.ty)
                {
                    return state.lower_float_comparison(
                        kind, comparison, receiver, argument, span, layer_sink,
                    );
                }
                state
                    .lower_named_call_on_receiver(receiver, &name, call, layer_sink, None, required)
            },
        ) {
            self.commit_expr_layer(layer, sink)
        } else {
            let receiver = self.lower_expr(receiver, sink, None)?;
            if let Some(comparison) = comparison
                && let Some(kind) = self.float_kind(receiver.ty)
            {
                return self
                    .lower_float_comparison(kind, comparison, receiver, argument, span, sink);
            }
            self.lower_named_call_on_receiver(
                receiver,
                &name,
                call,
                sink,
                None,
                RequiredCallableModifiers {
                    operator: Some(kind),
                    infix: false,
                    ..Default::default()
                },
            )?
        };
        if let Some(comparison) = comparison {
            if matches!(value.kind, ExprKind::FloatBinary { .. }) {
                return Some(value);
            }
            let long = self.integer_type(hir::IntegerKind::SIGNED_64);
            debug_assert_eq!(value.ty, long);
            return Some(hir::Expr {
                kind: ExprKind::Binary {
                    op: comparison,
                    lhs: Box::new(value),
                    rhs: Box::new(hir::Expr {
                        kind: ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(0)),
                        ty: long,
                        span,
                        origin: self.expression_origin(span),
                    }),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            });
        }
        Some(if negate {
            hir::Expr {
                kind: ExprKind::Unary {
                    op: hir::UnOp::Not,
                    operand: Box::new(value),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            }
        } else {
            value
        })
    }

    pub(super) fn lower_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if matches!(op, ast::BinOp::And | ast::BinOp::Or) {
            return self.lower_short_circuit(op, lhs, rhs, span, sink);
        }
        if matches!(
            op,
            ast::BinOp::Add
                | ast::BinOp::Sub
                | ast::BinOp::Mul
                | ast::BinOp::Div
                | ast::BinOp::Rem
                | ast::BinOp::RangeTo
                | ast::BinOp::RangeUntil
                | ast::BinOp::Contains
                | ast::BinOp::NotContains
                | ast::BinOp::Lt
                | ast::BinOp::Le
                | ast::BinOp::Gt
                | ast::BinOp::Ge
        ) {
            return self.lower_conventional_binary(op, lhs, rhs, span, sink, expected);
        }
        // `===` / `!==` (spec 4.4.2): reference identity, only on
        // reference types; on value types it is a compile error.
        if matches!(op, ast::BinOp::RefEq | ast::BinOp::RefNe) {
            return self.lower_ref_eq(op, lhs, rhs, span, sink);
        }
        debug_assert!(matches!(op, ast::BinOp::Eq | ast::BinOp::Ne));
        let negate = op == ast::BinOp::Ne;
        let symbol = if negate { "!=" } else { "==" };
        // A contextual operand takes its exact type from the independently
        // typed peer. Commit the peer's type arena together with its expression,
        // while retaining source evaluation order in the two operand sinks.
        let lhs_is_integer_literal = crate::expr::integer_literal_default_kind(lhs).is_some();
        let rhs_is_integer_literal = crate::expr::integer_literal_default_kind(rhs).is_some();
        let direct_integer_kind = crate::expr::common_integer_literal_kind(&[lhs, rhs]);
        let (lhs, rhs) = if let Some(kind) = direct_integer_kind {
            let expected = self.integer_type(kind);
            let lhs = self.lower_expr(lhs, sink, Some(expected))?;
            let rhs = self.lower_expr(rhs, sink, Some(expected))?;
            (lhs, rhs)
        } else if lhs_is_integer_literal && !rhs_is_integer_literal {
            // Integer literal syntax is side-effect free. Typing the other
            // operand first therefore preserves evaluation semantics while
            // letting the closed integer family provide the exact context.
            let rhs = self.lower_expr(rhs, sink, None)?;
            let lhs_expected = match self.types[rhs.ty] {
                Type::Integer(kind) if crate::expr::integer_literal_accepts_kind(lhs, kind) => {
                    Some(rhs.ty)
                }
                _ => None,
            };
            let lhs = self.lower_expr(lhs, sink, lhs_expected)?;
            (lhs, rhs)
        } else {
            let mut operands = [None, None];
            let mut failures = [None, None];
            for (index, source) in [lhs, rhs].into_iter().enumerate() {
                operands[index] = self.probe_contextual_value(
                    self.expr_context_requirement(source),
                    None,
                    &mut failures[index],
                    |this, expected| {
                        let mut setup = Vec::new();
                        this.lower_expr(source, &mut setup, expected)
                            .map(|value| (value, setup))
                    },
                )?;
            }
            if operands[0].is_none() {
                let hint = operands[1].as_ref().map(|(value, _)| value.ty);
                let mut setup = Vec::new();
                let value =
                    self.finish_contextual_value(hint, &mut failures[0], |this, expected| {
                        this.lower_expr(lhs, &mut setup, expected)
                    })?;
                operands[0] = Some((value, setup));
            }
            let (lhs, lhs_setup) = operands[0].take().expect("the left operand is complete");
            if operands[1].is_none() {
                let hint = if rhs_is_integer_literal {
                    match self.types[lhs.ty] {
                        Type::Integer(kind)
                            if crate::expr::integer_literal_accepts_kind(rhs, kind) =>
                        {
                            Some(lhs.ty)
                        }
                        _ => None,
                    }
                } else {
                    Some(lhs.ty)
                };
                let mut setup = Vec::new();
                let value =
                    self.finish_contextual_value(hint, &mut failures[1], |this, expected| {
                        this.lower_expr(rhs, &mut setup, expected)
                    })?;
                operands[1] = Some((value, setup));
            }
            let (rhs, rhs_setup) = operands[1].take().expect("the right operand is complete");
            sink.extend(lhs_setup);
            let lhs = if rhs_setup.is_empty() {
                lhs
            } else {
                let span = lhs.span;
                self.materialize_temporary("$equality.lhs".into(), lhs, span, sink)
            };
            sink.extend(rhs_setup);
            (lhs, rhs)
        };
        self.lower_equality_operator(negate, lhs, rhs, symbol, span, sink)
    }

    /// `===` / `!==` (spec 4.4.2): both operands must be reference
    /// types (class / interface / `Any` / `String` / arrays); on value
    /// types it is a compile error. This is the explicit identity
    /// operator and is unrelated to ordinary `operator equals` lookup.
    pub(super) fn lower_ref_eq(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let symbol = if op == ast::BinOp::RefEq {
            "==="
        } else {
            "!=="
        };
        let lhs_expected = self.function_smart_cast_storage(lhs).map(|(_, ty)| ty);
        let lhs = self.lower_expr(lhs, sink, lhs_expected)?;
        let rhs_expected = self.function_smart_cast_storage(rhs).map(|(_, ty)| ty);
        let (lhs, rhs) = self.lower_ordered_rhs(lhs, rhs, sink, rhs_expected, "$equality.lhs")?;
        if !self.is_ref_ty(lhs.ty) || !self.is_ref_ty(rhs.ty) {
            self.error(
                span,
                format!("reference equality `{symbol}` is not supported on value types"),
            );
            return None;
        }
        let op = if op == ast::BinOp::RefEq {
            hir::BinOp::RefEq
        } else {
            hir::BinOp::RefNe
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        })
    }

    pub(super) fn lower_unary(
        &mut self,
        op: ast::UnOp,
        operand: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
            && let ast::Expr::FloatLiteral(literal) = operand
        {
            return self.lower_float_literal(literal, expected, op == ast::UnOp::Neg, span);
        }
        if op == ast::UnOp::Neg
            && let ast::Expr::IntLiteral(literal) = operand
            && matches!(
                literal.suffix,
                ast::IntegerSuffix::None | ast::IntegerSuffix::Long
            )
        {
            return self.lower_integer_literal(*literal, expected, true, span);
        }
        let (kind, name) = match op {
            ast::UnOp::Plus => (hir::OperatorKind::UnaryPlus, "unaryPlus"),
            ast::UnOp::Neg => (hir::OperatorKind::UnaryMinus, "unaryMinus"),
            ast::UnOp::Not => (hir::OperatorKind::Not, "not"),
        };
        let name = ast::Ident {
            text: name.to_string(),
            span,
        };
        let call = CallSite {
            type_args: &[],
            args: &[],
            span,
        };
        let required = RequiredCallableModifiers {
            operator: Some(kind),
            infix: false,
            ..Default::default()
        };
        if matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
            && crate::expr::integer_literal_candidate_kinds(operand).is_some()
        {
            if let Some(layer) = self.probe_numeric_literal_receiver(
                operand,
                expected,
                |state, receiver, layer_sink| {
                    state.lower_named_call_on_receiver(
                        receiver, &name, call, layer_sink, None, required,
                    )
                },
            ) {
                return Some(self.commit_expr_layer(layer, sink));
            }
            let receiver = self.lower_expr(operand, sink, None)?;
            return self.lower_named_call_on_receiver(receiver, &name, call, sink, None, required);
        }
        let receiver_expected = matches!(op, ast::UnOp::Plus | ast::UnOp::Neg)
            .then_some(expected)
            .flatten();
        let receiver = self.lower_expr(operand, sink, receiver_expected)?;
        self.lower_named_call_on_receiver(receiver, &name, call, sink, None, required)
    }
}
