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
                hir::StatementKind::Return { value }
                    if self.types_equal(target, self.unit)
                        && !self.types_equal(value.ty, self.unit)
                        && !self.is_nothing_ty(value.ty) =>
                {
                    out.push(hir::Statement {
                        kind: hir::StatementKind::Expr(value),
                        span: statement.span,
                    });
                    statement.kind = hir::StatementKind::Return {
                        value: self.unit_expression(statement.span),
                    };
                }
                hir::StatementKind::Return { value } => {
                    statement.kind = hir::StatementKind::Return {
                        value: self.adapt_to(value, target),
                    };
                }
                hir::StatementKind::ContextScope { value, body } => {
                    statement.kind = hir::StatementKind::ContextScope {
                        value,
                        body: self.adapt_inferred_returns(body, target),
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
                    loop_kind,
                    target: loop_target,
                    condition_setup,
                    cond,
                    body,
                } => {
                    statement.kind = hir::StatementKind::While {
                        loop_kind,
                        target: loop_target,
                        condition_setup: self.adapt_inferred_returns(condition_setup, target),
                        cond,
                        body: self.adapt_inferred_returns(body, target),
                    };
                }
                hir::StatementKind::When(mut when) => {
                    for arm in when.arms_mut() {
                        if let hir::WhenConditionMut::Predicate(condition) = arm.condition {
                            condition.setup = self.adapt_inferred_returns(
                                std::mem::take(&mut condition.setup),
                                target,
                            );
                        }
                        if let Some(guard) = arm.guard {
                            guard.setup = self
                                .adapt_inferred_returns(std::mem::take(&mut guard.setup), target);
                        }
                        *arm.body = self.adapt_inferred_returns(std::mem::take(arm.body), target);
                    }
                    if let Some(body) = when.else_body_mut() {
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
        let name = self.source_function_declarations.get(&id).map_or_else(
            || self.functions[id].name.clone(),
            |source| source.name.clone(),
        );
        self.with_function_body(id, &name, generate)
    }

    pub(crate) fn lower_generated_body(
        &mut self,
        id: FunctionId,
        block: &ast::Block,
        source_file: usize,
    ) -> hir::Body {
        let name = self.source_function_declarations[&id].name.clone();
        self.with_function_body(id, &name, |lowerer| {
            let previous_source = lowerer.generated_source_file.replace(source_file);
            let previous_context = lowerer.current_source_context;
            lowerer.set_source_context(hir::SourceContextSubject::Function(id));
            let statements = lowerer.lower_function_block(id, &name, block);
            lowerer.generated_source_file = previous_source;
            lowerer.current_source_context = previous_context;
            statements
        })
    }

    fn lower_function_block(
        &mut self,
        id: FunctionId,
        name: &str,
        block: &ast::Block,
    ) -> Vec<hir::Statement> {
        let returns_unit = self.types_equal(self.signatures[&id].return_ty, self.unit);
        let diagnostics_before = self.diagnostics.len();
        let statements = self.lower_block(block);
        // A non-Unit block must return on every reachable path. Earlier
        // errors already explain incomplete statement lowering.
        if !returns_unit
            && self.diagnostics.len() == diagnostics_before
            && self
                .statements_control_outcomes(&statements)
                .can_fall_through()
        {
            self.error(
                block.span,
                format!("non-Unit function `{name}` may complete without returning a value"),
            );
        }
        statements
    }

    fn lower_body_input(
        &mut self,
        id: FunctionId,
        name: &str,
        input: &ast::FunctionBody,
    ) -> Vec<hir::Statement> {
        let return_ty = self.signatures[&id].return_ty;
        match input {
            ast::FunctionBody::Block(block) => self.lower_function_block(id, name, block),
            // `fun f(...) [: T] = expr` is a single `return expr`.
            ast::FunctionBody::Expr(expr) => {
                let mut statements = Vec::new();
                let mut sink = Vec::new();
                let inferring = self.signatures[&id].result_pending;
                if !self.reject_expression_body_return(expr)
                    && let Some(value) =
                        self.lower_expr(expr, &mut sink, (!inferring).then_some(return_ty))
                {
                    let return_ty = if inferring {
                        self.set_inferred_function_result(id, value.ty);
                        value.ty
                    } else {
                        return_ty
                    };
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

    /// Preserve the checked result expression; a source bare return has the
    /// unique Unit value at the return statement's evaluation origin.
    pub(super) fn push_return(
        &mut self,
        value: Option<hir::Expr>,
        span: Span,
        out: &mut Vec<hir::Statement>,
    ) {
        let value = value.unwrap_or_else(|| self.unit_expression(span));
        out.push(hir::Statement {
            kind: hir::StatementKind::Return { value },
            span,
        });
    }
}
