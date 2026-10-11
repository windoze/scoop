//! Shared Boolean short-circuit evaluation and setup placement.

use super::*;

impl Lowerer {
    pub(super) fn lower_short_circuit(
        &mut self,
        source_op: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let lhs_ast = lhs;
        let before = self.diagnostics.len();
        let lhs = self.lower_expr(lhs_ast, sink, None)?;
        let rhs_outcome = source_op == ast::BinOp::And;
        let narrowings = if self.diagnostics.len() == before {
            self.resolve_smart_casts(lhs_ast, rhs_outcome)
        } else {
            Default::default()
        };
        let mut rhs_setup = Vec::new();
        let rhs = self.with_smart_casts(narrowings, |this| {
            this.lower_expr(rhs, &mut rhs_setup, None)
        })?;
        let symbol = if source_op == ast::BinOp::And {
            "&&"
        } else {
            "||"
        };
        if !self.is_subtype(lhs.ty, self.boolean) || !self.is_subtype(rhs.ty, self.boolean) {
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
        let op = if source_op == ast::BinOp::And {
            hir::BinOp::And
        } else {
            hir::BinOp::Or
        };
        Some(self.finish_short_circuit(op, lhs, rhs, rhs_setup, span, sink))
    }

    pub(crate) fn finish_short_circuit(
        &mut self,
        op: hir::BinOp,
        lhs: hir::Expr,
        rhs: hir::Expr,
        rhs_setup: Vec<hir::Statement>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        if rhs_setup.is_empty() {
            return hir::Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: self.boolean,
                span,
                origin: self.expression_origin(span),
            };
        }

        let result = self.alloc_hidden("shortCircuit", self.boolean);
        let result_decl = |value| hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: value,
            },
            span,
        };
        let literal = |value| hir::Expr {
            kind: ExprKind::BoolLiteral(value),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        };
        let mut rhs_branch = rhs_setup;
        rhs_branch.push(result_decl(rhs));
        let (then_body, else_body) = if op == hir::BinOp::And {
            (rhs_branch, vec![result_decl(literal(false))])
        } else {
            (vec![result_decl(literal(true))], rhs_branch)
        };
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: lhs,
                then_body,
                else_body: Some(else_body),
            },
            span,
        });
        hir::Expr {
            kind: ExprKind::Local(result),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        }
    }
}
