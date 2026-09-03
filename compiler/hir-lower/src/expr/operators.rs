use super::*;

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
            },
        )
    }

    pub(crate) fn component_operator_indices(
        &mut self,
        receiver_ty: TypeId,
    ) -> Vec<std::num::NonZeroU32> {
        let mut indices = self
            .signatures
            .values()
            .filter_map(|signature| match signature.modifiers.operator {
                Some(hir::OperatorKind::Component { index }) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        indices.sort_unstable();
        indices.dedup();
        indices.retain(|&index| {
            let operator = hir::OperatorKind::Component { index };
            !self.methods_by_operator(receiver_ty, operator).is_empty()
                || self
                    .extension_operator_candidate_layers(operator)
                    .into_iter()
                    .flatten()
                    .any(|function| {
                        let extension_ty = self.extension_receivers[&function];
                        matches!(self.types[extension_ty], Type::Param(_))
                            || self.is_subtype(receiver_ty, extension_ty)
                    })
        });
        indices
    }

    pub(super) fn lower_conventional_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
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
        let receiver = self.lower_expr(receiver, sink, None)?;
        let name = ast::Ident {
            text: name.to_string(),
            span,
        };
        let args = [ast::CallArgument::positional(argument.clone())];
        let value = self.lower_named_call_on_receiver(
            receiver,
            &name,
            CallSite {
                type_args: &[],
                args: &args,
                span,
            },
            sink,
            None,
            RequiredCallableModifiers {
                operator: Some(kind),
                infix: false,
            },
        )?;
        if let Some(comparison) = comparison {
            debug_assert_eq!(value.ty, self.int);
            return Some(hir::Expr {
                kind: ExprKind::Binary {
                    op: comparison,
                    lhs: Box::new(value),
                    rhs: Box::new(hir::Expr {
                        kind: ExprKind::IntLiteral(0),
                        ty: self.int,
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
            return self.lower_conventional_binary(op, lhs, rhs, span, sink);
        }
        // `===` / `!==` (spec 4.4.2): reference identity, only on
        // reference types; on value types it is a compile error.
        if matches!(op, ast::BinOp::RefEq | ast::BinOp::RefNe) {
            return self.lower_ref_eq(op, lhs, rhs, span, sink);
        }
        debug_assert!(matches!(op, ast::BinOp::Eq | ast::BinOp::Ne));
        let negate = op == ast::BinOp::Ne;
        let symbol = if negate { "!=" } else { "==" };
        // `x == None` / `None == x`: the `None` construction takes its
        // type from the other operand (expected-type hint), so the
        // other side is lowered first. (`None` itself is side-effect
        // free, so lowering order is unobservable here.)
        let (lhs, rhs) = if is_none_literal(lhs) && !is_none_literal(rhs) {
            let rhs = self.lower_expr(rhs, sink, None)?;
            let lhs = self.lower_expr(lhs, sink, Some(rhs.ty))?;
            (lhs, rhs)
        } else {
            let lhs = self.lower_expr(lhs, sink, None)?;
            let rhs_hint = if is_none_literal(rhs) {
                Some(lhs.ty)
            } else {
                None
            };
            let rhs = self.lower_expr(rhs, sink, rhs_hint)?;
            (lhs, rhs)
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

    /// Resolve `==` / `!=` through the lhs static type's ordinary
    /// `operator fun equals` member set. The operands arrive already lowered,
    /// preserving the language's left-to-right, exactly-once evaluation rule;
    /// applicability and MSC still use the same overload engine as an explicit
    /// member call. Nominal value derivation contributes a typed synthetic
    /// candidate whose application carries its complete ordinary HIR body.
    pub(super) fn lower_equality_operator(
        &mut self,
        negate: bool,
        lhs: hir::Expr,
        rhs: hir::Expr,
        symbol: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let mut candidates = self.methods_by_name(lhs.ty, "equals");
        candidates.retain(|candidate| {
            self.signatures[&candidate.function].modifiers.operator
                == Some(hir::OperatorKind::Equals)
        });
        let mut derived = None;
        let mut structural_derived = None;
        let mut derivation_failure = None;
        if self.types_equal(lhs.ty, rhs.ty) {
            match self.derived_equality_candidate(lhs.ty, span) {
                Ok(Some(crate::derived::DerivedEqualityCandidate::Nominal {
                    overload,
                    application,
                })) => {
                    derived = Some((overload.function, application));
                    candidates.push(overload);
                }
                Ok(Some(crate::derived::DerivedEqualityCandidate::Structural {
                    function,
                    application,
                })) => structural_derived = Some((function, application)),
                Ok(None) => {}
                Err(reason) => derivation_failure = Some(reason),
            }
        }
        if let Some((function, application)) = structural_derived {
            debug_assert!(candidates.is_empty());
            self.check_call_effects(hir::Callable::Function(function), span);
            let call = hir::Expr {
                kind: ExprKind::MethodCall {
                    receiver: Box::new(lhs),
                    callee: hir::MethodCallee::DerivedEquality(application),
                    args: vec![rhs],
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            };
            return Some(if negate {
                hir::Expr {
                    kind: ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(call),
                    },
                    ty: self.boolean,
                    span,
                    origin: self.expression_origin(span),
                }
            } else {
                call
            });
        }
        if !candidates.is_empty() {
            let resolved = self.resolve_member_overload_lowered(
                "equals",
                &candidates,
                crate::overload::LoweredOverloadCall {
                    explicit_type_args: Vec::new(),
                    args: vec![rhs],
                    span,
                    expected_result: None,
                },
                sink,
            )?;
            debug_assert_eq!(resolved.return_ty, self.boolean);
            let function = resolved.function();
            self.check_call_effects(hir::Callable::Function(function), span);
            let callee = match derived {
                Some((derived_function, application)) if function == derived_function => {
                    hir::MethodCallee::DerivedEquality(application)
                }
                _ => {
                    let callee = self.materialize_resolved_callee(&resolved);
                    self.materialize_method_callee(resolved.source, callee, &resolved.type_args)
                }
            };
            let call = hir::Expr {
                kind: ExprKind::MethodCall {
                    receiver: Box::new(lhs),
                    callee,
                    args: resolved.args,
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            };
            return Some(if negate {
                hir::Expr {
                    kind: ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(call),
                    },
                    ty: self.boolean,
                    span,
                    origin: self.expression_origin(span),
                }
            } else {
                call
            });
        }

        if let Some(reason) = derivation_failure {
            let found = self.type_name(lhs.ty);
            self.error(
                span,
                format!("cannot derive `equals` for `{found}`: {reason}"),
            );
            return None;
        }

        let found = self.type_name(lhs.ty);
        self.error(
            span,
            format!("type `{found}` has no member operator `equals` for `{symbol}`"),
        );
        None
    }

    /// Resolve the equality operation used by a literal pattern while the
    /// matched subject type is still explicit. Literal patterns are limited
    /// to primitive/String literals, so this is always an ordinary core
    /// member call; the exact callable crosses HIR instead of being selected
    /// again from the literal kind in MIR.
    pub(crate) fn resolve_literal_pattern_equality(
        &mut self,
        subject_ty: hir::TypeId,
        literal: hir::Expr,
        span: Span,
    ) -> Option<(hir::Expr, hir::Callable)> {
        let candidates = self
            .methods_by_name(subject_ty, "equals")
            .into_iter()
            .filter(|candidate| {
                self.signatures[&candidate.function].modifiers.operator
                    == Some(hir::OperatorKind::Equals)
            })
            .collect::<Vec<_>>();
        let mut sink = Vec::new();
        let resolved = self.resolve_member_overload_lowered(
            "equals",
            &candidates,
            crate::overload::LoweredOverloadCall {
                explicit_type_args: Vec::new(),
                args: vec![literal],
                span,
                expected_result: None,
            },
            &mut sink,
        )?;
        debug_assert!(
            sink.is_empty(),
            "literal equality with identical static types needs no temporary"
        );
        let callable = self.materialize_resolved_callee(&resolved);
        self.check_call_effects(callable, span);
        let callee = self.materialize_method_callee(resolved.source, callable, &resolved.type_args);
        let hir::MethodCallee::Callable(callee) = callee else {
            unreachable!("literal core equality is an ordinary concrete member")
        };
        let [literal] = resolved.args.as_slice() else {
            unreachable!("equals has exactly one explicit argument")
        };
        Some((literal.clone(), callee))
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
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(operand, sink, None)?;
        let (kind, name) = match op {
            ast::UnOp::Plus => (hir::OperatorKind::UnaryPlus, "unaryPlus"),
            ast::UnOp::Neg => (hir::OperatorKind::UnaryMinus, "unaryMinus"),
            ast::UnOp::Not => (hir::OperatorKind::Not, "not"),
        };
        self.lower_named_call_on_receiver(
            receiver,
            &ast::Ident {
                text: name.to_string(),
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
                operator: Some(kind),
                infix: false,
            },
        )
    }
}
