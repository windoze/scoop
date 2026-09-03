use super::*;

impl Lowerer {
    pub(super) fn lower_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        // `===` / `!==` (spec 4.4.2): reference identity, only on
        // reference types; on value types it is a compile error.
        if matches!(op, ast::BinOp::RefEq | ast::BinOp::RefNe) {
            return self.lower_ref_eq(op, lhs, rhs, span, sink);
        }
        let (op, symbol) = convert_bin_op(op);
        // `x == None` / `None == x`: the `None` construction takes its
        // type from the other operand (expected-type hint), so the
        // other side is lowered first. (`None` itself is side-effect
        // free, so lowering order is unobservable here.)
        let (lhs, rhs) = if matches!(op, hir::BinOp::Eq | hir::BinOp::Ne)
            && is_none_literal(lhs)
            && !is_none_literal(rhs)
        {
            let rhs = self.lower_expr(rhs, sink, None)?;
            let lhs = self.lower_expr(lhs, sink, Some(rhs.ty))?;
            (lhs, rhs)
        } else if op == hir::BinOp::And {
            // `x is T && ...`: the right side is only evaluated when
            // the left holds, so its smart-cast narrowings apply there
            // (milestone6 DESIGN.md 5.4).
            let lhs_ast = lhs;
            let before = self.diagnostics.len();
            let lhs = self.lower_expr(lhs_ast, sink, None)?;
            let narrowings = if self.diagnostics.len() == before {
                self.resolve_smart_casts(lhs_ast, true)
            } else {
                Vec::new()
            };
            let rhs = self.with_smart_casts(narrowings, |this| this.lower_expr(rhs, sink, None))?;
            (lhs, rhs)
        } else {
            let lhs = self.lower_expr(lhs, sink, None)?;
            let rhs_hint = if matches!(op, hir::BinOp::Eq | hir::BinOp::Ne) && is_none_literal(rhs)
            {
                Some(lhs.ty)
            } else {
                None
            };
            let rhs = self.lower_expr(rhs, sink, rhs_hint)?;
            (lhs, rhs)
        };
        if matches!(op, hir::BinOp::Eq | hir::BinOp::Ne) {
            return self.lower_equality_operator(op, lhs, rhs, symbol, span, sink);
        }
        if matches!(op, hir::BinOp::Add | hir::BinOp::Sub)
            && matches!(self.types[lhs.ty], Type::Ptr(_))
        {
            if rhs.ty != self.int {
                self.error(
                    span,
                    format!(
                        "pointer operator `{symbol}` requires an Int offset, found {}",
                        self.type_name(rhs.ty)
                    ),
                );
                return None;
            }
            self.require_unsafe_operation(span, "pointer arithmetic");
            let ty = lhs.ty;
            return Some(hir::Expr {
                kind: ExprKind::PtrOffset {
                    pointer: Box::new(lhs),
                    offset: Box::new(rhs),
                    subtract: op == hir::BinOp::Sub,
                },
                ty,
                span,
            });
        }
        let ty = match op {
            hir::BinOp::Add => {
                // `String + String` concatenates (docs/milestone2/
                // DESIGN.md 2.3); all other arithmetic is numeric
                // (Int, or UInt since M9).
                if lhs.ty == self.string && rhs.ty == self.string {
                    self.string
                } else {
                    self.expect_numeric_operands(symbol, &lhs, &rhs, span)?
                }
            }
            hir::BinOp::Sub | hir::BinOp::Mul | hir::BinOp::Div => {
                self.expect_numeric_operands(symbol, &lhs, &rhs, span)?
            }
            hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge => {
                self.expect_numeric_operands(symbol, &lhs, &rhs, span)?;
                self.boolean
            }
            hir::BinOp::Eq | hir::BinOp::Ne | hir::BinOp::RefEq | hir::BinOp::RefNe => {
                unreachable!("equality operators are lowered before primitive binary operators")
            }
            hir::BinOp::And | hir::BinOp::Or => {
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
                self.boolean
            }
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            span,
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
        op: hir::BinOp,
        lhs: hir::Expr,
        rhs: hir::Expr,
        symbol: &str,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let mut candidates = self.methods_by_name(lhs.ty, "equals");
        candidates.retain(|candidate| {
            self.signatures[&candidate.function].operator == Some(hir::OperatorKind::Equals)
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
            };
            return Some(if op == hir::BinOp::Ne {
                hir::Expr {
                    kind: ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(call),
                    },
                    ty: self.boolean,
                    span,
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
            self.check_call_effects(resolved.callee, span);
            let function = self.callable_function_id(resolved.callee);
            let callee = match derived {
                Some((derived_function, application)) if function == derived_function => {
                    hir::MethodCallee::DerivedEquality(application)
                }
                _ => self.materialize_method_callee(
                    resolved.source,
                    resolved.callee,
                    &resolved.type_args,
                ),
            };
            let call = hir::Expr {
                kind: ExprKind::MethodCall {
                    receiver: Box::new(lhs),
                    callee,
                    args: resolved.args,
                },
                ty: self.boolean,
                span,
            };
            return Some(if op == hir::BinOp::Ne {
                hir::Expr {
                    kind: ExprKind::Unary {
                        op: hir::UnOp::Not,
                        operand: Box::new(call),
                    },
                    ty: self.boolean,
                    span,
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
                self.signatures[&candidate.function].operator == Some(hir::OperatorKind::Equals)
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
        self.check_call_effects(resolved.callee, span);
        let callee =
            self.materialize_method_callee(resolved.source, resolved.callee, &resolved.type_args);
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
        })
    }

    /// Arithmetic and comparison operators require both operands to
    /// share one numeric type and return it (`Int` or — M9,
    /// milestone9 DESIGN.md section 1 — `UInt`, under the same rules;
    /// comparisons yield `Boolean` at the call site). Mixing `Int`
    /// and `UInt` is an error: the types are distinct and there is no
    /// implicit conversion (spec 11.2). Type parameters are rejected
    /// too: `T` is unconstrained, so no operation beyond `==` / `!=`
    /// can be proven valid at the definition site (DESIGN.md 2.2).
    /// The unsigned semantics risks of `UInt` arithmetic (subtraction
    /// underflow, signed-vs-unsigned comparison) are deferred to a
    /// later milestone; the machine word wraps for now.
    pub(super) fn expect_numeric_operands(
        &mut self,
        symbol: &str,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> Option<TypeId> {
        if lhs.ty == self.int && rhs.ty == self.int {
            return Some(self.int);
        }
        if lhs.ty == self.uint && rhs.ty == self.uint {
            return Some(self.uint);
        }
        let lhs_ty = self.type_name(lhs.ty);
        let rhs_ty = self.type_name(rhs.ty);
        let message = if lhs.ty == self.uint || rhs.ty == self.uint {
            format!(
                "operator `{symbol}` requires Int or UInt operands of the same type, found {lhs_ty} and {rhs_ty}"
            )
        } else {
            format!("operator `{symbol}` requires Int operands, found {lhs_ty} and {rhs_ty}")
        };
        self.error(span, message);
        None
    }

    pub(super) fn lower_unary(
        &mut self,
        op: ast::UnOp,
        operand: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let operand = self.lower_expr(operand, sink, None)?;
        let (op, symbol, expected, ty) = match op {
            ast::UnOp::Neg => (hir::UnOp::Neg, "-", self.int, self.int),
            ast::UnOp::Not => (hir::UnOp::Not, "!", self.boolean, self.boolean),
        };
        if operand.ty != expected {
            let article = if expected == self.int { "an" } else { "a" };
            let expected_name = self.type_name(expected);
            let found = self.type_name(operand.ty);
            self.error(
                span,
                format!(
                    "operator `{symbol}` requires {article} {expected_name} operand, found {found}"
                ),
            );
            return None;
        }
        Some(hir::Expr {
            kind: ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            ty,
            span,
        })
    }
}
