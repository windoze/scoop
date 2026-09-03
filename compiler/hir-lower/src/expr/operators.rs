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
        if self.uses_legacy_binary_operator(op, receiver.ty)
            && !self.type_exposes_operator(receiver.ty, kind)
        {
            return self.lower_legacy_conventional_binary(op, receiver, argument, span, sink);
        }
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

    fn type_exposes_operator(&mut self, ty: TypeId, kind: hir::OperatorKind) -> bool {
        let name = operator_source_name(kind);
        if self
            .methods_by_name(ty, name)
            .into_iter()
            .any(|candidate| self.signatures[&candidate.function].modifiers.operator == Some(kind))
        {
            return true;
        }
        self.extension_candidate_layers(name)
            .into_iter()
            .flatten()
            .any(|function| self.signatures[&function].modifiers.operator == Some(kind))
    }

    /// Compatibility bridge for the core declarations migrated in slice 7.
    /// User-defined capability never enters this branch: it is selected above
    /// through the typed operator role and the ordinary overload resolver.
    fn lower_legacy_conventional_binary(
        &mut self,
        op: ast::BinOp,
        receiver: hir::Expr,
        argument: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let pointer = matches!(self.types[receiver.ty], Type::Ptr(_));
        let argument = self.lower_expr(argument, sink, None)?;
        let (hir_op, symbol) = match op {
            ast::BinOp::Add => (hir::BinOp::Add, "+"),
            ast::BinOp::Sub => (hir::BinOp::Sub, "-"),
            ast::BinOp::Mul => (hir::BinOp::Mul, "*"),
            ast::BinOp::Div => (hir::BinOp::Div, "/"),
            ast::BinOp::Rem => (hir::BinOp::Rem, "%"),
            ast::BinOp::Lt => (hir::BinOp::Lt, "<"),
            ast::BinOp::Le => (hir::BinOp::Le, "<="),
            ast::BinOp::Gt => (hir::BinOp::Gt, ">"),
            ast::BinOp::Ge => (hir::BinOp::Ge, ">="),
            _ => unreachable!("the compatibility bridge accepts a closed operator set"),
        };
        if pointer {
            if argument.ty != self.int {
                self.error(
                    span,
                    format!(
                        "pointer operator `{symbol}` requires an Int offset, found {}",
                        self.type_name(argument.ty)
                    ),
                );
                return None;
            }
            self.require_unsafe_operation(span, "pointer arithmetic");
            let ty = receiver.ty;
            return Some(hir::Expr {
                kind: ExprKind::PtrOffset {
                    pointer: Box::new(receiver),
                    offset: Box::new(argument),
                    subtract: op == ast::BinOp::Sub,
                },
                ty,
                span,
                origin: self.expression_origin(span),
            });
        }
        let ty = if hir_op == hir::BinOp::Add
            && receiver.ty == self.string
            && argument.ty == self.string
        {
            self.string
        } else {
            self.expect_numeric_operands(symbol, &receiver, &argument, span)?;
            if matches!(
                hir_op,
                hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge
            ) {
                self.boolean
            } else {
                receiver.ty
            }
        };
        Some(hir::Expr {
            kind: ExprKind::Binary {
                op: hir_op,
                lhs: Box::new(receiver),
                rhs: Box::new(argument),
            },
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn uses_legacy_binary_operator(&self, op: ast::BinOp, receiver: TypeId) -> bool {
        let scalar_or_parameter = matches!(
            self.types[receiver],
            Type::Int | Type::UInt | Type::Boolean | Type::String | Type::Param(_)
        );
        scalar_or_parameter
            && matches!(
                op,
                ast::BinOp::Add
                    | ast::BinOp::Sub
                    | ast::BinOp::Mul
                    | ast::BinOp::Div
                    | ast::BinOp::Rem
                    | ast::BinOp::Lt
                    | ast::BinOp::Le
                    | ast::BinOp::Gt
                    | ast::BinOp::Ge
            )
            || matches!(self.types[receiver], Type::Ptr(_))
                && matches!(op, ast::BinOp::Add | ast::BinOp::Sub)
    }

    pub(super) fn lower_binary(
        &mut self,
        op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
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
        let ty = match op {
            hir::BinOp::Add
            | hir::BinOp::Sub
            | hir::BinOp::Mul
            | hir::BinOp::Div
            | hir::BinOp::Rem
            | hir::BinOp::Lt
            | hir::BinOp::Le
            | hir::BinOp::Gt
            | hir::BinOp::Ge => {
                unreachable!("overloadable operators use the conventional call path")
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
        op: hir::BinOp,
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
            return Some(if op == hir::BinOp::Ne {
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
            return Some(if op == hir::BinOp::Ne {
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
        let receiver = self.lower_expr(operand, sink, None)?;
        let (kind, name, legacy) = match op {
            ast::UnOp::Plus => (
                hir::OperatorKind::UnaryPlus,
                "unaryPlus",
                (receiver.ty == self.int || receiver.ty == self.uint).then_some(None),
            ),
            ast::UnOp::Neg => (
                hir::OperatorKind::UnaryMinus,
                "unaryMinus",
                (receiver.ty == self.int).then_some(Some(hir::UnOp::Neg)),
            ),
            ast::UnOp::Not => (
                hir::OperatorKind::Not,
                "not",
                (receiver.ty == self.boolean).then_some(Some(hir::UnOp::Not)),
            ),
        };
        if matches!(
            self.types[receiver.ty],
            Type::Int | Type::UInt | Type::Boolean | Type::String | Type::Param(_)
        ) && !self.type_exposes_operator(receiver.ty, kind)
        {
            if let Some(legacy) = legacy {
                return Some(match legacy {
                    None => receiver,
                    Some(op) => hir::Expr {
                        ty: receiver.ty,
                        kind: ExprKind::Unary {
                            op,
                            operand: Box::new(receiver),
                        },
                        span,
                        origin: self.expression_origin(span),
                    },
                });
            }
            let (symbol, expected, article) = match op {
                ast::UnOp::Plus => ("+", "Int", "an"),
                ast::UnOp::Neg => ("-", "Int", "an"),
                ast::UnOp::Not => ("!", "Boolean", "a"),
            };
            self.error(
                span,
                format!(
                    "operator `{symbol}` requires {article} {expected} operand, found {}",
                    self.type_name(receiver.ty)
                ),
            );
            return None;
        }
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

fn operator_source_name(kind: hir::OperatorKind) -> &'static str {
    match kind {
        hir::OperatorKind::UnaryPlus => "unaryPlus",
        hir::OperatorKind::UnaryMinus => "unaryMinus",
        hir::OperatorKind::Not => "not",
        hir::OperatorKind::Inc => "inc",
        hir::OperatorKind::Dec => "dec",
        hir::OperatorKind::Plus => "plus",
        hir::OperatorKind::Minus => "minus",
        hir::OperatorKind::Times => "times",
        hir::OperatorKind::Div => "div",
        hir::OperatorKind::Rem => "rem",
        hir::OperatorKind::RangeTo => "rangeTo",
        hir::OperatorKind::RangeUntil => "rangeUntil",
        hir::OperatorKind::Contains => "contains",
        hir::OperatorKind::Get => "get",
        hir::OperatorKind::Set => "set",
        hir::OperatorKind::Invoke => "invoke",
        hir::OperatorKind::PlusAssign => "plusAssign",
        hir::OperatorKind::MinusAssign => "minusAssign",
        hir::OperatorKind::TimesAssign => "timesAssign",
        hir::OperatorKind::DivAssign => "divAssign",
        hir::OperatorKind::RemAssign => "remAssign",
        hir::OperatorKind::CompareTo => "compareTo",
        hir::OperatorKind::Equals => "equals",
        hir::OperatorKind::Component { .. } => "componentN",
        hir::OperatorKind::Iterator => "iterator",
    }
}
