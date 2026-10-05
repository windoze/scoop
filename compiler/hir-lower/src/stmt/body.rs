use super::*;

mod context;

impl Lowerer {
    pub(crate) fn adapt_inferred_returns(
        &mut self,
        statements: Vec<hir::Statement>,
        target: TypeId,
    ) -> Vec<hir::Statement> {
        let mut out = Vec::with_capacity(statements.len());
        for mut statement in statements {
            match statement.kind {
                hir::StatementKind::Return { value: Some(value) }
                    if self.types_equal(target, self.unit) =>
                {
                    out.push(hir::Statement {
                        kind: hir::StatementKind::Expr(value),
                        span: statement.span,
                    });
                    statement.kind = hir::StatementKind::Return { value: None };
                }
                hir::StatementKind::Return { value: Some(value) } => {
                    statement.kind = hir::StatementKind::Return {
                        value: Some(self.adapt_to(value, target)),
                    };
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    statement.kind = hir::StatementKind::If {
                        cond,
                        then_body: self.adapt_inferred_returns(then_body, target),
                        else_body: else_body.map(|body| self.adapt_inferred_returns(body, target)),
                    };
                }
                hir::StatementKind::While {
                    target: loop_target,
                    condition_setup,
                    cond,
                    body,
                } => {
                    statement.kind = hir::StatementKind::While {
                        target: loop_target,
                        condition_setup: self.adapt_inferred_returns(condition_setup, target),
                        cond,
                        body: self.adapt_inferred_returns(body, target),
                    };
                }
                hir::StatementKind::When(mut when) => {
                    for arm in &mut when.arms {
                        arm.body =
                            self.adapt_inferred_returns(std::mem::take(&mut arm.body), target);
                    }
                    if let hir::WhenFallback::Else(body) = &mut when.fallback {
                        *body = self.adapt_inferred_returns(std::mem::take(body), target);
                    }
                    statement.kind = hir::StatementKind::When(when);
                }
                hir::StatementKind::Try(mut try_) => {
                    try_.body = self.adapt_inferred_returns(try_.body, target);
                    for catch in &mut try_.catches {
                        catch.body =
                            self.adapt_inferred_returns(std::mem::take(&mut catch.body), target);
                    }
                    try_.finally_body = try_
                        .finally_body
                        .take()
                        .map(|body| self.adapt_inferred_returns(body, target));
                    statement.kind = hir::StatementKind::Try(try_);
                }
                _ => {}
            }
            out.push(statement);
        }
        out
    }

    pub(crate) fn lower_body(&mut self, id: FunctionId, decl: &ast::FunctionDecl) -> hir::Body {
        self.with_function_body(id, &decl.name.text, |lowerer| {
            lowerer.lower_body_input(id, &decl.name.text, &decl.body)
        })
    }

    pub(crate) fn lower_synthesized_body(
        &mut self,
        id: FunctionId,
        generate: impl FnOnce(&mut Self) -> Vec<hir::Statement>,
    ) -> hir::Body {
        let name = self.source_function_declarations[&id].name.clone();
        self.with_function_body(id, &name, generate)
    }

    fn lower_body_input(
        &mut self,
        id: FunctionId,
        name: &str,
        input: &ast::FunctionBody,
    ) -> Vec<hir::Statement> {
        let return_ty = self.signatures[&id].return_ty;
        let returns_unit = self.types_equal(return_ty, self.unit);
        match input {
            ast::FunctionBody::Block(block) => {
                let diagnostics_before = self.diagnostics.len();
                let statements = self.lower_block(block);
                // A non-Unit block body must not fall through on any
                // reachable path. The analysis is compositional over
                // sequential statements and structured control flow;
                // skipped when lowering already diagnosed the body,
                // because missing statements are commonly fallout of
                // an earlier error.
                if !returns_unit
                    && self.diagnostics.len() == diagnostics_before
                    && statements_control_outcomes(&statements).can_fall_through()
                {
                    self.error(
                        block.span,
                        format!(
                            "non-Unit function `{name}` may complete without returning a value"
                        ),
                    );
                }
                statements
            }
            // `fun f(...) [: T] = expr` is a single `return expr`.
            ast::FunctionBody::Expr(expr) => {
                let mut statements = Vec::new();
                let mut sink = Vec::new();
                if let Some(value) = self.lower_expr(expr, &mut sink, Some(return_ty)) {
                    if !self.is_subtype(value.ty, return_ty) {
                        let expected = self.type_name(return_ty);
                        let found = self.type_name(value.ty);
                        let message = self.with_nominal_invariance_detail(
                            format!("body of `{name}` must be of type {expected}, found {found}"),
                            value.ty,
                            return_ty,
                        );
                        self.error(expr.span(), message);
                    } else {
                        let value = self.adapt_to(value, return_ty);
                        statements.extend(sink);
                        self.push_return(Some(value), expr.span(), &mut statements);
                    }
                }
                statements
            }
            // Bodyless declarations were diagnosed at signature
            // resolution (abstract / interface / intrinsic only);
            // there is nothing to lower.
            ast::FunctionBody::None => Vec::new(),
        }
    }

    /// Append the statement(s) returning `value` (already checked
    /// against the current function's return type). In a `Unit`
    /// function the value is evaluated as a plain statement and the
    /// `return` is bare: `hir::StatementKind::Return::value` is absent
    /// in `Unit` functions (hir docs).
    pub(super) fn push_return(
        &mut self,
        value: Option<hir::Expr>,
        span: Span,
        out: &mut Vec<hir::Statement>,
    ) {
        if let Some(value) = value {
            if self.types_equal(self.current_return_ty, self.unit) {
                out.push(hir::Statement {
                    kind: hir::StatementKind::Expr(value),
                    span,
                });
                out.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: None },
                    span,
                });
            } else {
                out.push(hir::Statement {
                    kind: hir::StatementKind::Return { value: Some(value) },
                    span,
                });
            }
        } else {
            out.push(hir::Statement {
                kind: hir::StatementKind::Return { value: None },
                span,
            });
        }
    }
}
