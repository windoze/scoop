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
    pub(crate) fn lower_statement(
        &mut self,
        statement: &ast::Statement,
        out: &mut Vec<hir::Statement>,
    ) {
        if self.body_cancelled() {
            return;
        }
        let incoming = self.smart_casts.clone();
        let start = out.len();
        self.lower_statement_inner(statement, out);
        self.advance_smart_casts(incoming, &out[start..], None);
    }

    fn lower_statement_inner(&mut self, statement: &ast::Statement, out: &mut Vec<hir::Statement>) {
        if self.reject_release_statement(statement) {
            return;
        }
        let kind = match &statement.kind {
            ast::StatementKind::Expr(expr) => {
                let mut sink = Vec::new();
                let Some(lowered) = self.lower_expr(expr, &mut sink, None) else {
                    return; // diagnostic already recorded
                };
                if matches!(expr, ast::Expr::ContextScope { .. }) {
                    // The structured scope evaluates its tail before restoring
                    // the binding; statement position discards that value.
                    out.extend(sink);
                    return;
                }
                // Calls and updates may discard their result; declarations,
                // assignments and control flow have their own statement kinds.
                // Constructions lower to `StructInit` /
                // `VariantConstruct`, not `Call`, so they are rejected
                // here.
                if self.is_nothing_ty(lowered.ty)
                    || matches!(expr, ast::Expr::Update { .. })
                    // Safe calls execute inside their desugaring sink and
                    // leave only an Option-valued local as the expression.
                    || matches!(
                        expr,
                        ast::Expr::MethodCall {
                            navigation: ast::Navigation::Safe,
                            ..
                        }
                    )
                    || matches!(
                        lowered.kind,
                        hir::ExprKind::Call { .. }
                            | hir::ExprKind::MethodCall { .. }
                            | hir::ExprKind::DirectSuperMethodCall { .. }
                            | hir::ExprKind::CallableCall { .. }
                            | hir::ExprKind::AnnotationKey(_)
                            | hir::ExprKind::Atomic(_)
                            | hir::ExprKind::MaybeUninit(_)
                            | hir::ExprKind::PtrStore { .. }
                            | hir::ExprKind::ForeignCallbackRegister { .. }
                            | hir::ExprKind::ForeignCallbackOperation { .. }
                    )
                {
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
            ast::StatementKind::TypeAlias(declaration) => {
                self.declare_local_type_alias(declaration);
                return;
            }
            ast::StatementKind::LocalType(declaration) => {
                self.lower_local_type(declaration, false);
                return;
            }
            ast::StatementKind::LocalFunction(function) => {
                let Some(local) = self.lower_local_function_decl(function, out) else {
                    return;
                };
                hir::StatementKind::LocalFunction(local)
            }
            ast::StatementKind::Return { label, value } => {
                self.lower_return(label.as_ref(), value.as_ref(), statement.span, out);
                return;
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
                let (then_narrowings, else_narrowings) = if self.diagnostics.len() == before {
                    (
                        self.checked_condition_facts(&cond, true),
                        self.checked_condition_facts(&cond, false),
                    )
                } else {
                    (Default::default(), Default::default())
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
            ast::StatementKind::While(loop_) => {
                let Some(kind) = self.lower_loop(loop_) else {
                    return;
                };
                kind
            }
            ast::StatementKind::For(for_) => {
                if let Some(statements) = self.lower_for(for_) {
                    out.extend(statements);
                }
                return;
            }
            ast::StatementKind::Break { label } => {
                let Some(jump) =
                    self.lower_loop_jump(ast::LoopJumpKind::Break, label.as_ref(), statement.span)
                else {
                    return;
                };
                jump
            }
            ast::StatementKind::Continue { label } => {
                let Some(jump) = self.lower_loop_jump(
                    ast::LoopJumpKind::Continue,
                    label.as_ref(),
                    statement.span,
                ) else {
                    return;
                };
                jump
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
        // A trivial named declaration needs no separate subject/projection
        // schedule. Composite and wildcard targets use the shared binding
        // planner, which expands to binding-only HIR declarations in source
        // runtime order. Planning happens after the initializer, so none of
        // the new bindings are visible in their own initializer.
        if matches!(&decl.target, ast::Pattern::Binding(_)) {
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
        } else {
            let plan =
                self.lower_irrefutable_binding(&decl.target, init, decl.mutable, decl.span)?;
            out.extend(sink);
            out.extend(plan);
        }
        Some(())
    }

    /// `if` / `while` conditions must be `Boolean`.
    pub(crate) fn lower_condition(
        &mut self,
        cond: &ast::Expr,
        keyword: &str,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let cond = self.lower_expr(cond, sink, None)?;
        if !self.is_subtype(cond.ty, self.boolean) {
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
