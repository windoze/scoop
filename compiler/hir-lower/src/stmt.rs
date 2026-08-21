//! Statement lowering: declarations, assignments, control flow and
//! block-level scoping (milestone2 DESIGN.md 2.2).

use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn lower_body(&mut self, decl: &ast::FunctionDecl) -> hir::Body {
        let statements = self.lower_block(&decl.body);
        hir::Body {
            locals: std::mem::take(&mut self.locals),
            statements,
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
    fn lower_statement(&mut self, statement: &ast::Statement, out: &mut Vec<hir::Statement>) {
        let kind = match &statement.kind {
            ast::StatementKind::Expr(expr) => {
                let Some(lowered) = self.lower_expr(expr) else {
                    return; // diagnostic already recorded
                };
                // M1 rule, unchanged: expression statements are calls
                // (declarations, assignments and control flow are their
                // own statement kinds since M2).
                if matches!(lowered.kind, hir::ExprKind::Call { .. }) {
                    hir::StatementKind::Expr(lowered)
                } else {
                    self.error(
                        statement.span,
                        "statement must be a function call".to_string(),
                    );
                    return;
                }
            }
            ast::StatementKind::ValDecl(decl) => {
                let Some(kind) = self.lower_val_decl(decl) else {
                    return;
                };
                kind
            }
            ast::StatementKind::Assign(assign) => {
                let Some(kind) = self.lower_assign(assign) else {
                    return;
                };
                kind
            }
            ast::StatementKind::If(if_) => {
                let Some(cond) = self.lower_condition(&if_.cond, "if") else {
                    return;
                };
                let then_body = self.lower_block(&if_.then_block);
                let else_body = if_.else_block.as_ref().map(|b| self.lower_block(b));
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            ast::StatementKind::While(while_) => {
                let Some(cond) = self.lower_condition(&while_.cond, "while") else {
                    return;
                };
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
    /// otherwise.
    fn lower_val_decl(&mut self, decl: &ast::ValDecl) -> Option<hir::StatementKind> {
        let annotation = match &decl.ty {
            Some(ty_ref) => Some(self.resolve_type_ref(ty_ref)?),
            None => None,
        };
        let init = self.lower_expr(&decl.init)?;
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
        Some(hir::StatementKind::ValDecl { local, init })
    }

    /// Assignment targets a declared, mutable local; the value type
    /// must match the local's type.
    fn lower_assign(&mut self, assign: &ast::Assign) -> Option<hir::StatementKind> {
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
        let value = self.lower_expr(&assign.value)?;
        let expected = self.locals[local].ty;
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
        Some(hir::StatementKind::Assign { local, value })
    }

    /// `if` / `while` conditions must be `Boolean`.
    fn lower_condition(&mut self, cond: &ast::Expr, keyword: &str) -> Option<hir::Expr> {
        let cond = self.lower_expr(cond)?;
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
