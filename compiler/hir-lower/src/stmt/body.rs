use super::*;

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
                hir::StatementKind::While { cond, body } => {
                    statement.kind = hir::StatementKind::While {
                        cond,
                        body: self.adapt_inferred_returns(body, target),
                    };
                }
                hir::StatementKind::When(mut when) => {
                    for arm in &mut when.arms {
                        arm.body =
                            self.adapt_inferred_returns(std::mem::take(&mut arm.body), target);
                    }
                    when.else_body = when
                        .else_body
                        .take()
                        .map(|body| self.adapt_inferred_returns(body, target));
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
        let sig = self.signatures[&id].clone();
        // Member functions (M6): `this` is parameter 0, an immutable
        // local of the host type; bare property / method names in the
        // body resolve against it.
        let owner = self.function_owner.get(&id).copied();
        // Method signatures already carry the combined owner-prefix plus
        // method-suffix namespace established in pass 2.5.
        self.type_params_in_scope = sig.type_params.clone();
        self.current_return_ty = sig.return_ty;
        self.current_fn_name = decl.name.text.clone();
        self.push_suspension_context(if decl.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
        });
        self.push_safety_context(self.functions[id].attributes.safety);

        self.current_owner = owner;
        self.current_this = None;

        // Parameters are immutable locals in the function's outermost
        // scope; the body block nests inside it, so body locals may
        // shadow parameters.
        self.push_scope();
        let extension_receiver = self.extension_receivers.get(&id).copied();
        let mut params = Vec::with_capacity(
            sig.params.len() + usize::from(owner.is_some() || extension_receiver.is_some()),
        );
        if let Some(host_ty) = owner
            .map(|owner| self.owner_ty(owner))
            .or(extension_receiver)
        {
            let local = self.alloc_local("this".to_string(), host_ty, false);
            self.scopes.declare("this".to_string(), local);
            self.current_this = Some((local, host_ty));
            params.push(hir::Param {
                name: "this".to_string(),
                ty: host_ty,
                local,
            });
        }
        for param in &sig.params {
            if self.scopes.is_declared_here(&param.name.text) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.text),
                );
                continue;
            }
            let local = self.alloc_local(param.name.text.clone(), param.ty, false);
            self.scopes.declare(param.name.text.clone(), local);
            params.push(hir::Param {
                name: param.name.text.clone(),
                ty: param.ty,
                local,
            });
        }
        self.functions[id].params = params;

        let returns_unit = self.types_equal(sig.return_ty, self.unit);
        let statements = match &decl.body {
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
                    && statements_can_fall_through(&statements)
                {
                    self.error(
                        block.span,
                        format!(
                            "non-Unit function `{}` may complete without returning a value",
                            decl.name.text
                        ),
                    );
                }
                statements
            }
            // `fun f(...) [: T] = expr` is a single `return expr`.
            ast::FunctionBody::Expr(expr) => {
                let mut statements = Vec::new();
                let mut sink = Vec::new();
                if let Some(value) = self.lower_expr(expr, &mut sink, Some(sig.return_ty)) {
                    if !self.is_subtype(value.ty, sig.return_ty) {
                        let expected = self.type_name(sig.return_ty);
                        let found = self.type_name(value.ty);
                        self.error(
                            expr.span(),
                            format!(
                                "body of `{}` must be of type {expected}, found {found}",
                                decl.name.text
                            ),
                        );
                    } else {
                        let value = self.adapt_to(value, sig.return_ty);
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
        };
        self.pop_scope();
        self.type_params_in_scope.clear();
        self.current_this = None;
        self.current_owner = None;
        self.pop_safety_context();
        self.pop_suspension_context();

        hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements,
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
