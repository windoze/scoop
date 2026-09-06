use super::*;

impl Lowerer {
    /// Lower one statement, appending to `out`. Nested blocks are
    /// flattened into `out`: scoping is fully resolved here (every
    /// reference already carries its `LocalId`), so HIR needs no block
    /// statement.
    ///
    /// Expression positions create a fresh desugaring sink (see
    /// `lower_expr` in expr.rs); the sink is drained into `out` right
    /// before the owning statement.
    pub(super) fn lower_statement(
        &mut self,
        statement: &ast::Statement,
        out: &mut Vec<hir::Statement>,
    ) {
        let kind = match &statement.kind {
            ast::StatementKind::Expr(expr) => {
                let mut sink = Vec::new();
                let Some(lowered) = self.lower_expr(expr, &mut sink, None) else {
                    return; // diagnostic already recorded
                };
                // M1 rule, unchanged: expression statements are calls
                // (declarations, assignments and control flow are their
                // own statement kinds since M2; M6 adds method calls).
                // Constructions lower to `StructInit` /
                // `VariantConstruct`, not `Call`, so they are rejected
                // here.
                if matches!(
                    lowered.kind,
                    hir::ExprKind::Call { .. }
                        | hir::ExprKind::MethodCall { .. }
                        | hir::ExprKind::LocalFunctionCall { .. }
                        | hir::ExprKind::CallableCall { .. }
                        | hir::ExprKind::PtrStore { .. }
                        | hir::ExprKind::ForeignCallbackOperation {
                            operation: hir::ForeignCallbackOperation::Release,
                            ..
                        }
                ) {
                    out.extend(sink);
                    hir::StatementKind::Expr(lowered)
                } else {
                    self.error(
                        statement.span,
                        "statement must be a function call".to_string(),
                    );
                    return;
                }
            }
            ast::StatementKind::LocalFunction(function) => {
                let Some(local) = self.lower_local_function_decl(function) else {
                    return;
                };
                hir::StatementKind::LocalFunction(local)
            }
            ast::StatementKind::Return { value } => {
                if self.initialization_context.is_some() {
                    self.error(
                        statement.span,
                        "`return` is not allowed in an initializer or constructor body".into(),
                    );
                    return;
                }
                if self.return_inference.is_some() {
                    let kind = match value {
                        None => {
                            self.return_inference
                                .as_mut()
                                .expect("checked above")
                                .saw_bare = true;
                            hir::StatementKind::Return { value: None }
                        }
                        Some(expr) => {
                            let mut sink = Vec::new();
                            let Some(value) = self.lower_expr(expr, &mut sink, None) else {
                                return;
                            };
                            self.return_inference
                                .as_mut()
                                .expect("checked above")
                                .value_types
                                .push(value.ty);
                            out.extend(sink);
                            hir::StatementKind::Return { value: Some(value) }
                        }
                    };
                    out.push(hir::Statement {
                        kind,
                        span: statement.span,
                    });
                    return;
                }
                let return_ty = self.current_return_ty;
                match value {
                    None => {
                        if !self.types_equal(return_ty, self.unit) {
                            let name = self.current_fn_name.clone();
                            let expected = self.type_name(return_ty);
                            self.error(
                                statement.span,
                                format!(
                                    "`return` without a value in function `{name}` returning {expected}"
                                ),
                            );
                            return;
                        }
                        hir::StatementKind::Return { value: None }
                    }
                    Some(expr) => {
                        let mut sink = Vec::new();
                        let Some(value) = self.lower_expr(expr, &mut sink, Some(return_ty)) else {
                            return; // diagnostic already recorded
                        };
                        if !self.is_subtype(value.ty, return_ty) {
                            let name = self.current_fn_name.clone();
                            let expected = self.type_name(return_ty);
                            let found = self.type_name(value.ty);
                            let message = self.with_nominal_invariance_detail(
                                format!(
                                    "`return` value of `{name}` must be of type {expected}, found {found}"
                                ),
                                value.ty,
                                return_ty,
                            );
                            self.error(expr.span(), message);
                            return;
                        }
                        let value = self.adapt_to(value, return_ty);
                        out.extend(sink);
                        self.push_return(Some(value), statement.span, out);
                        return; // push_return already appended
                    }
                }
            }
            ast::StatementKind::ValDecl(decl) => {
                self.lower_val_decl(decl, out);
                return;
            }
            ast::StatementKind::LocalDelegatedProperty(decl) => {
                self.lower_local_delegated_property(decl, out);
                return;
            }
            ast::StatementKind::When(when) => {
                let Some(kind) = self.lower_when(when, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Throw(expr) => {
                let Some(kind) = self.lower_throw(expr, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Try(try_) => self.lower_try(try_),
            ast::StatementKind::Assign(assign) => {
                let Some(kind) = self.lower_assign(assign, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::If(if_) => {
                let mut sink = Vec::new();
                let before = self.diagnostics.len();
                let Some(cond) = self.lower_condition(&if_.cond, "if", &mut sink) else {
                    return;
                };
                // The condition is evaluated exactly once, right before
                // the `if`, so desugaring statements belong before it.
                out.extend(sink);
                // Smart casts (milestone6 DESIGN.md 5.4): `x is T`
                // narrows `x` in the then branch, `x !is T` in the
                // else branch. Skipped when the condition produced
                // diagnostics (its narrowings would be unreliable; the
                // module is rejected anyway).
                let (then_narrowings, else_narrowings) = if self.diagnostics.len() == before {
                    (
                        self.resolve_smart_casts(&if_.cond, true),
                        self.resolve_smart_casts(&if_.cond, false),
                    )
                } else {
                    (Vec::new(), Vec::new())
                };
                let then_body = self
                    .with_smart_casts(then_narrowings, |this| this.lower_block(&if_.then_block));
                let else_body = if_
                    .else_block
                    .as_ref()
                    .map(|b| self.with_smart_casts(else_narrowings, |this| this.lower_block(b)));
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            ast::StatementKind::While(while_) => {
                let mut sink = Vec::new();
                let Some(cond) = self.lower_condition(&while_.cond, "while", &mut sink) else {
                    return;
                };
                let body = self.lower_block(&while_.body);
                hir::StatementKind::While {
                    condition_setup: sink,
                    cond,
                    body,
                }
            }
            ast::StatementKind::Block(block) => {
                self.push_scope();
                for statement in &block.statements {
                    self.lower_statement(statement, out);
                }
                self.pop_scope();
                return;
            }
            ast::StatementKind::SafetyBlock { mode, block } => {
                let safety = match mode {
                    ast::SafetyMode::Safe => hir::Safety::Safe,
                    ast::SafetyMode::Unsafe => hir::Safety::Unsafe,
                };
                self.push_safety_context(safety);
                self.push_scope();
                for statement in &block.statements {
                    self.lower_statement(statement, out);
                }
                self.pop_scope();
                self.pop_safety_context();
                return;
            }
        };
        out.push(hir::Statement {
            kind,
            span: statement.span,
        });
    }

    /// `val` / `var` declarations always have an initializer (the AST
    /// guarantees it): the declared type is the annotation when present
    /// (the initializer must match it exactly), the initializer's type
    /// otherwise. The annotation is the initializer's expected-type
    /// hint (this is what types a `None` construction).
    ///
    /// The binding target is a pattern (spec 4.6): a plain `val x` is
    /// `Pattern::Binding`, destructuring uses tuple/struct patterns.
    /// Only irrefutable patterns are allowed here — `lower_pattern`
    /// with `in_when: false` rejects enum variants and literals.
    fn lower_val_decl(&mut self, decl: &ast::ValDecl, out: &mut Vec<hir::Statement>) -> Option<()> {
        let annotation = match &decl.ty {
            Some(ty_ref) => Some(self.resolve_type_ref(ty_ref)?),
            None => None,
        };
        let mut sink = Vec::new();
        let init = self.lower_expr(&decl.init, &mut sink, annotation)?;
        let (init, ty) = match annotation {
            Some(expected) => {
                if !self.is_subtype(init.ty, expected) {
                    let expected_name = self.type_name(expected);
                    let found = self.type_name(init.ty);
                    let message = self.with_nominal_invariance_detail(
                        format!(
                            "initializer of `{}` must be of type {expected_name}, found {found}",
                            ast::dump_pattern(&decl.target)
                        ),
                        init.ty,
                        expected,
                    );
                    self.error(decl.init.span(), message);
                    return None;
                }
                (self.adapt_to(init, expected), expected)
            }
            None => {
                let ty = init.ty;
                (init, ty)
            }
        };
        if let (
            Type::Class(_),
            ast::Pattern::Tuple {
                elements,
                rest,
                span,
            },
        ) = (self.types[ty].clone(), &decl.target)
        {
            let diagnostics_before = self.diagnostics.len();
            let mut state = self.clone();
            let mut planned = Vec::new();
            let lowered = state.lower_class_destructuring(
                elements,
                *rest,
                *span,
                decl.mutable,
                init,
                sink,
                &mut planned,
            );
            if lowered.is_some() && state.diagnostics.len() == diagnostics_before {
                *self = state;
                out.extend(planned);
                return Some(());
            }
            self.diagnostics
                .extend(state.diagnostics.into_iter().skip(diagnostics_before));
            return None;
        }
        // The pattern is lowered after the initializer, so bindings are
        // not visible in their own initializer.
        let pattern = self.lower_pattern(
            &decl.target,
            ty,
            PatternCtx {
                mutable: decl.mutable,
                in_when: false,
            },
        )?;
        out.extend(sink);
        out.push(hir::Statement {
            kind: hir::StatementKind::ValDecl { pattern, init },
            span: decl.span,
        });
        Some(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_class_destructuring(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        mutable: bool,
        init: hir::Expr,
        mut statements: Vec<hir::Statement>,
        out: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        let receiver_ty = init.ty;
        let indices = self.component_operator_indices(receiver_ty);
        let total = indices.last().map_or(0, |index| index.get() as usize);
        let owner = format!("class `{}`", self.type_name(receiver_ty));
        let positions = self.positional_pattern_indices(elements, rest, total, &owner, span)?;

        let receiver = self.alloc_hidden("subject", receiver_ty);
        statements.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: receiver },
                init,
            },
            span,
        });
        let origin = self.expression_origin(span);
        for (element, position) in elements.iter().zip(positions) {
            let index = std::num::NonZeroU32::new((position + 1) as u32)
                .expect("class component indices start at one");
            let component = self.lower_component_call(
                hir::Expr {
                    kind: hir::ExprKind::Local(receiver),
                    ty: receiver_ty,
                    span,
                    origin,
                },
                index,
                span,
                &mut statements,
            )?;
            let pattern = self.lower_pattern(
                element,
                component.ty,
                PatternCtx {
                    mutable,
                    in_when: false,
                },
            )?;
            statements.push(hir::Statement {
                kind: hir::StatementKind::ValDecl {
                    pattern,
                    init: component,
                },
                span,
            });
        }
        out.extend(statements);
        Some(())
    }

    /// `if` / `while` conditions must be `Boolean`.
    pub(super) fn lower_condition(
        &mut self,
        cond: &ast::Expr,
        keyword: &str,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let cond = self.lower_expr(cond, sink, None)?;
        if cond.ty != self.boolean {
            let found = self.type_name(cond.ty);
            self.error(
                cond.span,
                format!("{keyword} condition must be Boolean, found {found}"),
            );
            return None;
        }
        Some(cond)
    }
}
