use super::*;
use crate::expr::members::ImportedMemberSelectionFailure;
use crate::expr::named_calls::imported_dependency::{ImportedMemberReceiver, ImportedProbeCall};

mod fields;

impl Lowerer {
    /// Finish the left evaluation before any statements produced by the right.
    pub(in crate::expr) fn lower_equality_rhs(
        &mut self,
        lhs: hir::Expr,
        rhs: &ast::Expr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<(hir::Expr, hir::Expr)> {
        let mut rhs_sink = Vec::new();
        let rhs = self.lower_expr(rhs, &mut rhs_sink, expected)?;
        let lhs = if rhs_sink.is_empty() {
            lhs
        } else {
            let span = lhs.span;
            self.materialize_temporary("$equality.lhs".into(), lhs, span, sink)
        };
        sink.extend(rhs_sink);
        Some((lhs, rhs))
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
                Ok(Some(crate::derived::DerivedEqualityCandidate::Imported(target))) => {
                    if self.requires_unsafe_use(lhs.ty) {
                        self.require_unsafe_operation(
                            span,
                            "calling an unsafe dependency function",
                        );
                    }
                    let call = self.imported_equality_call(target, lhs, rhs, span);
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
                Ok(Some(crate::derived::DerivedEqualityCandidate::Nominal {
                    overload,
                    application,
                })) => {
                    derived = Some((overload.function, application));
                    candidates.push(overload);
                }
                Ok(Some(crate::derived::DerivedEqualityCandidate::TypeOwned {
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
            let derived_callee = match derived {
                Some((derived_function, application)) if function == derived_function => {
                    Some(hir::MethodCallee::DerivedEquality(application))
                }
                _ => None,
            };
            let call = if let Some(callee) = derived_callee {
                hir::Expr {
                    kind: ExprKind::MethodCall {
                        receiver: Box::new(lhs),
                        callee,
                        args: resolved.args,
                    },
                    ty: self.boolean,
                    span,
                    origin: self.expression_origin(span),
                }
            } else if let Some(normalized) = self.normalize_primitive_method_call(
                function,
                lhs.clone(),
                &resolved.args,
                self.boolean,
                span,
            ) {
                normalized
            } else {
                let callee = self.materialize_resolved_callee(&resolved);
                let callee =
                    self.materialize_method_callee(resolved.source, callee, &resolved.type_args);
                hir::Expr {
                    kind: ExprKind::MethodCall {
                        receiver: Box::new(lhs),
                        callee,
                        args: resolved.args,
                    },
                    ty: self.boolean,
                    span,
                    origin: self.expression_origin(span),
                }
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

        if let Some(probe) = self
            .select_imported_equality(ImportedMemberReceiver::Value(lhs.clone()), &rhs, span)
            .ok()?
        {
            let call = self.commit_imported_dependency_callable(probe, sink)?;
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
    /// matched subject type is still explicit. Integer `equals` is normalized
    /// to its typed representation-level plan here; Boolean and String retain
    /// the exact ordinary callable chosen by overload resolution.
    pub(crate) fn resolve_literal_pattern_equality(
        &mut self,
        subject_ty: hir::TypeId,
        literal: hir::Expr,
        span: Span,
    ) -> Option<(hir::Expr, hir::LiteralPatternEquality)> {
        let candidates = self
            .methods_by_name(subject_ty, "equals")
            .into_iter()
            .filter(|candidate| {
                self.signatures[&candidate.function].modifiers.operator
                    == Some(hir::OperatorKind::Equals)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty()
            && let Some(probe) = self
                .select_imported_equality(
                    ImportedMemberReceiver::LiteralSubject(subject_ty),
                    &literal,
                    span,
                )
                .ok()?
        {
            return self.commit_imported_literal_equality(probe);
        }
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
        let function = resolved.function();
        let integer_kind = match &self.functions[function].kind {
            hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind:
                    hir::IntrinsicFunctionKind::Integer(hir::IntegerIntrinsicKind::NoGcOperation {
                        kind,
                        operation: hir::NoGcIntegerOperation::Equals,
                    }),
                ..
            }) => Some(*kind),
            _ => None,
        };
        let equality = if matches!(
            self.functions[function].kind,
            hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
                kind: hir::IntrinsicFunctionKind::Char(hir::CharIntrinsic::Equals),
                ..
            })
        ) {
            self.check_call_effects(hir::Callable::Function(function), span);
            hir::LiteralPatternEquality::Char
        } else if let Some(kind) = integer_kind {
            self.check_call_effects(hir::Callable::Function(function), span);
            hir::LiteralPatternEquality::Integer { kind }
        } else if let hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind:
                hir::IntrinsicFunctionKind::Float(hir::FloatIntrinsicKind::Binary {
                    kind,
                    operation: hir::FloatBinaryOperator::Equal,
                }),
            ..
        }) = self.functions[function].kind
        {
            self.check_call_effects(hir::Callable::Function(function), span);
            hir::LiteralPatternEquality::Float { kind }
        } else {
            let callable = self.materialize_resolved_callee(&resolved);
            self.check_call_effects(callable, span);
            let callee =
                self.materialize_method_callee(resolved.source, callable, &resolved.type_args);
            let hir::MethodCallee::Callable(equals) = callee else {
                unreachable!("literal equality is an ordinary concrete member")
            };
            hir::LiteralPatternEquality::Ordinary { equals }
        };
        let [literal] = resolved.args.as_slice() else {
            unreachable!("equals has exactly one explicit argument")
        };
        Some((literal.clone(), equality))
    }

    fn select_imported_equality(
        &mut self,
        receiver: ImportedMemberReceiver,
        argument: &hir::Expr,
        span: Span,
    ) -> Result<Option<ImportedDependencyCallProbe>, ()> {
        let name = ast::Ident {
            text: "equals".into(),
            span,
        };
        match self.select_imported_member_probe(
            receiver,
            &name,
            ImportedProbeCall::lowered(std::slice::from_ref(argument), span),
            Some(self.boolean),
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Equals),
                ..Default::default()
            },
        ) {
            Ok(probe) => Ok(Some(probe)),
            Err(ImportedMemberSelectionFailure::NoApplicable(None)) => Ok(None),
            Err(
                ImportedMemberSelectionFailure::NoApplicable(Some(failure))
                | ImportedMemberSelectionFailure::Failed(failure),
            ) => {
                self.commit_layer_diagnostics(*failure);
                Err(())
            }
        }
    }
}
