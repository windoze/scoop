//! Statement lowering: declarations, assignments, control flow,
//! `return` and block-level scoping (milestone2 DESIGN.md 2.2,
//! milestone3 DESIGN.md 2.2).

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::FunctionId;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn lower_body(&mut self, id: FunctionId, decl: &ast::FunctionDecl) -> hir::Body {
        let sig = self.signatures[&id].clone();
        self.type_params_in_scope = sig.type_params.clone();
        self.current_return_ty = sig.return_ty;
        self.current_fn_name = decl.name.text.clone();

        // Parameters are immutable locals in the function's outermost
        // scope; the body block nests inside it, so body locals may
        // shadow parameters.
        self.scopes.push();
        let mut params = Vec::with_capacity(sig.params.len());
        for param in &sig.params {
            if self.scopes.is_declared_here(&param.name.text) {
                self.error(
                    param.name.span,
                    format!("duplicate parameter `{}`", param.name.text),
                );
                continue;
            }
            let local = self.locals.alloc(hir::Local {
                name: param.name.text.clone(),
                ty: param.ty,
                mutable: false,
            });
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
                // M3 simplified return rule (DESIGN.md 5.4): a non-Unit
                // block body must end with a `return` statement; branch
                // exhaustiveness is not analyzed. Skipped when the body
                // already produced diagnostics (the module is rejected
                // anyway, and a missing trailing statement is usually
                // just fallout of the earlier error).
                if !returns_unit
                    && self.diagnostics.len() == diagnostics_before
                    && !matches!(
                        statements.last().map(|s| &s.kind),
                        Some(hir::StatementKind::Return { .. })
                    )
                {
                    self.error(
                        block.span,
                        format!(
                            "non-Unit function `{}` must end with a return statement",
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
                    if !self.types_equal(sig.return_ty, value.ty) {
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
                        statements.extend(sink);
                        self.push_return(Some(value), expr.span(), &mut statements);
                    }
                }
                statements
            }
        };
        self.scopes.pop();
        self.type_params_in_scope.clear();

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
    fn push_return(&mut self, value: Option<hir::Expr>, span: Span, out: &mut Vec<hir::Statement>) {
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

    /// Lower a block in a fresh scope: declarations inside are not
    /// visible after the block ends.
    fn lower_block(&mut self, block: &ast::Block) -> Vec<hir::Statement> {
        self.scopes.push();
        let mut statements = Vec::new();
        for statement in &block.statements {
            self.lower_statement(statement, &mut statements);
        }
        self.scopes.pop();
        statements
    }

    /// Lower one statement, appending to `out`. Nested blocks are
    /// flattened into `out`: scoping is fully resolved here (every
    /// reference already carries its `LocalId`), so HIR needs no block
    /// statement.
    ///
    /// Expression positions create a fresh desugaring sink (see
    /// `lower_expr` in expr.rs); the sink is drained into `out` right
    /// before the owning statement.
    fn lower_statement(&mut self, statement: &ast::Statement, out: &mut Vec<hir::Statement>) {
        let kind = match &statement.kind {
            ast::StatementKind::Expr(expr) => {
                let mut sink = Vec::new();
                let Some(lowered) = self.lower_expr(expr, &mut sink, None) else {
                    return; // diagnostic already recorded
                };
                // M1 rule, unchanged: expression statements are calls
                // (declarations, assignments and control flow are their
                // own statement kinds since M2). Note `Some(...)`
                // lowers to `SomeWrap`, not `Call`, so it is rejected
                // here like struct constructions are.
                if matches!(lowered.kind, hir::ExprKind::Call { .. }) {
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
            ast::StatementKind::Return { value } => {
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
                        if !self.types_equal(return_ty, value.ty) {
                            let name = self.current_fn_name.clone();
                            let expected = self.type_name(return_ty);
                            let found = self.type_name(value.ty);
                            self.error(
                                expr.span(),
                                format!(
                                    "`return` value of `{name}` must be of type {expected}, found {found}"
                                ),
                            );
                            return;
                        }
                        out.extend(sink);
                        self.push_return(Some(value), statement.span, out);
                        return; // push_return already appended
                    }
                }
            }
            ast::StatementKind::ValDecl(decl) => {
                let Some(kind) = self.lower_val_decl(decl, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Assign(assign) => {
                let Some(kind) = self.lower_assign(assign, out) else {
                    return;
                };
                kind
            }
            ast::StatementKind::If(if_) => {
                let mut sink = Vec::new();
                let Some(cond) = self.lower_condition(&if_.cond, "if", &mut sink) else {
                    return;
                };
                // The condition is evaluated exactly once, right before
                // the `if`, so desugaring statements belong before it.
                out.extend(sink);
                let then_body = self.lower_block(&if_.then_block);
                let else_body = if_.else_block.as_ref().map(|b| self.lower_block(b));
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
                // A while condition is re-evaluated once per iteration,
                // but sink statements would execute once before the
                // loop; reject desugaring operators here instead of
                // silently changing evaluation semantics.
                if !sink.is_empty() {
                    self.error(
                        while_.span,
                        "`?.` and `?:` are not allowed in a while condition".to_string(),
                    );
                    return;
                }
                let body = self.lower_block(&while_.body);
                hir::StatementKind::While { cond, body }
            }
            ast::StatementKind::Block(block) => {
                self.scopes.push();
                for statement in &block.statements {
                    self.lower_statement(statement, out);
                }
                self.scopes.pop();
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
    /// hint (this is what types a `None` literal).
    fn lower_val_decl(
        &mut self,
        decl: &ast::ValDecl,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let annotation = match &decl.ty {
            Some(ty_ref) => Some(self.resolve_type_ref(ty_ref)?),
            None => None,
        };
        let mut sink = Vec::new();
        let init = self.lower_expr(&decl.init, &mut sink, annotation)?;
        let ty = match annotation {
            Some(expected) => {
                if !self.types_equal(expected, init.ty) {
                    let expected_name = self.type_name(expected);
                    let found = self.type_name(init.ty);
                    self.error(
                        decl.init.span(),
                        format!(
                            "initializer of `{}` must be of type {expected_name}, found {found}",
                            decl.name.text
                        ),
                    );
                    return None;
                }
                expected
            }
            None => init.ty,
        };
        if self.scopes.is_declared_here(&decl.name.text) {
            self.error(
                decl.name.span,
                format!("`{}` is already declared in this scope", decl.name.text),
            );
            return None;
        }
        let local = self.locals.alloc(hir::Local {
            name: decl.name.text.clone(),
            ty,
            mutable: decl.mutable,
        });
        self.scopes.declare(decl.name.text.clone(), local);
        out.extend(sink);
        Some(hir::StatementKind::ValDecl { local, init })
    }

    /// Assignment targets a declared, mutable local; the value type
    /// must match the local's type (which is also the value's
    /// expected-type hint).
    fn lower_assign(
        &mut self,
        assign: &ast::Assign,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let Some(local) = self.scopes.lookup(&assign.target.text) else {
            self.error(
                assign.target.span,
                format!("unknown variable `{}`", assign.target.text),
            );
            return None;
        };
        if !self.locals[local].mutable {
            self.error(
                assign.target.span,
                format!(
                    "cannot assign to immutable variable `{}`",
                    assign.target.text
                ),
            );
            return None;
        }
        let expected = self.locals[local].ty;
        let mut sink = Vec::new();
        let value = self.lower_expr(&assign.value, &mut sink, Some(expected))?;
        if !self.types_equal(expected, value.ty) {
            let expected_name = self.type_name(expected);
            let found = self.type_name(value.ty);
            self.error(
                assign.value.span(),
                format!(
                    "cannot assign value of type {found} to `{}` of type {expected_name}",
                    assign.target.text
                ),
            );
            return None;
        }
        out.extend(sink);
        Some(hir::StatementKind::Assign { local, value })
    }

    /// `if` / `while` conditions must be `Boolean`.
    fn lower_condition(
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
