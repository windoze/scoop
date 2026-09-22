use super::*;

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
        let value = if let Some(layer) = self.probe_integer_literal_receiver(
            receiver,
            expected,
            |state, receiver, layer_sink| {
                state
                    .lower_named_call_on_receiver(receiver, &name, call, layer_sink, None, required)
            },
        ) {
            self.commit_expr_layer(layer, sink)
        } else {
            let receiver = self.lower_expr(receiver, sink, None)?;
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
        // typed peer. A clone-only probe discovers that type; real lowering
        // still appends both operand sinks in source order because a payload
        // variant may evaluate effectful arguments.
        let lhs_is_integer_literal = crate::expr::integer_literal_default_kind(lhs).is_some();
        let rhs_is_integer_literal = crate::expr::integer_literal_default_kind(rhs).is_some();
        let lhs_requires_expected = self.expr_requires_expected_type(lhs);
        let rhs_requires_expected = self.expr_requires_expected_type(rhs);
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
        } else if lhs_requires_expected && !rhs_requires_expected {
            let mut probe = self.clone();
            let mut probe_sink = Vec::new();
            let Some(rhs_probe) = probe.lower_expr(rhs, &mut probe_sink, None) else {
                if probe.diagnostics.len() > self.diagnostics.len() {
                    self.commit_layer_diagnostics(probe);
                } else {
                    self.error(
                        rhs.span(),
                        "cannot determine equality operand type".to_string(),
                    );
                }
                return None;
            };
            let lhs = self.lower_expr(lhs, sink, Some(rhs_probe.ty))?;
            let rhs_expected = Some(lhs.ty);
            self.lower_equality_rhs(lhs, rhs, sink, rhs_expected)?
        } else {
            let lhs = self.lower_expr(lhs, sink, None)?;
            let rhs_hint = if rhs_is_integer_literal {
                match self.types[lhs.ty] {
                    Type::Integer(kind) if crate::expr::integer_literal_accepts_kind(rhs, kind) => {
                        Some(lhs.ty)
                    }
                    _ => None,
                }
            } else if rhs_requires_expected {
                Some(lhs.ty)
            } else {
                None
            };
            self.lower_equality_rhs(lhs, rhs, sink, rhs_hint)?
        };
        self.lower_equality_operator(negate, lhs, rhs, symbol, span, sink)
    }

    fn lower_short_circuit(
        &mut self,
        source_op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let lhs_ast = lhs;
        let before = self.diagnostics.len();
        let lhs = self.lower_expr(lhs_ast, sink, None)?;
        let rhs_outcome = source_op == ast::BinOp::And;
        let narrowings = if self.diagnostics.len() == before {
            self.resolve_smart_casts(lhs_ast, rhs_outcome)
        } else {
            Vec::new()
        };
        let mut rhs_setup = Vec::new();
        let rhs = self.with_smart_casts(narrowings, |this| {
            this.lower_expr(rhs, &mut rhs_setup, None)
        })?;
        let symbol = if source_op == ast::BinOp::And {
            "&&"
        } else {
            "||"
        };
        if lhs.ty != self.boolean || rhs.ty != self.boolean {
            let lhs_ty = self.type_name(lhs.ty);
            let rhs_ty = self.type_name(rhs.ty);
            self.error(
                span,
                format!(
                    "operator `{symbol}` requires Boolean operands, found {lhs_ty} and {rhs_ty}"
                ),
            );
            return None;
        }
        let op = if source_op == ast::BinOp::And {
            hir::BinOp::And
        } else {
            hir::BinOp::Or
        };
        if rhs_setup.is_empty() {
            return Some(hir::Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            });
        }

        let result = self.alloc_hidden("shortCircuit", self.boolean);
        let result_decl = |value| hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: value,
            },
            span,
        };
        let literal = |value| hir::Expr {
            kind: ExprKind::BoolLiteral(value),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        };
        let mut rhs_branch = rhs_setup;
        rhs_branch.push(result_decl(rhs));
        let (then_body, else_body) = if op == hir::BinOp::And {
            (rhs_branch, vec![result_decl(literal(false))])
        } else {
            (vec![result_decl(literal(true))], rhs_branch)
        };
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: lhs,
                then_body,
                else_body: Some(else_body),
            },
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        })
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
        let lhs = self.lower_expr(lhs, sink, None)?;
        let rhs = self.lower_expr(rhs, sink, None)?;
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
            if let Some(layer) = self.probe_integer_literal_receiver(
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
