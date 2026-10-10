use super::*;
use crate::stmt::ValueBlock;

impl Lowerer {
    /// Only normally completing Some/None paths provide the merged value.
    pub(super) fn lower_elvis(
        &mut self,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let lhs = self.lower_expr(lhs, sink, None)?;
        let Some(inner) = self.as_option(lhs.ty) else {
            let found = self.type_name(lhs.ty);
            self.error(
                span,
                format!("`?:` requires an Option left-hand side, found {found}"),
            );
            return None;
        };
        let subject = self.materialize_temporary("opt".into(), lhs, span, sink);
        let origin = self.expression_origin(span);
        let cond = hir::Expr {
            kind: ExprKind::IsSome(Box::new(subject.clone())),
            ty: self.boolean,
            span,
            origin,
        };
        let payload = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(subject),
                trap_on_none: false,
            },
            ty: inner,
            span,
            origin,
        };
        let mut then_body = self.expression_value_block(Vec::new(), payload);
        let mut else_setup = Vec::new();
        let rhs = self.lower_expr(rhs, &mut else_setup, expected.or(Some(inner)))?;
        let mut else_body = self.expression_value_block(else_setup, rhs);
        let result = self.finish_control_value(
            "Elvis",
            span,
            expected,
            &mut [&mut then_body, &mut else_body],
        )?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond,
                then_body: then_body.statements,
                else_body: Some(else_body.statements),
            },
            span,
        });
        Some(result)
    }

    fn expression_value_block(
        &self,
        mut statements: Vec<hir::Statement>,
        value: hir::Expr,
    ) -> ValueBlock {
        let value = if !self
            .statements_control_outcomes(&statements)
            .can_fall_through()
        {
            None
        } else if self.expression_can_complete(&value) {
            Some(value)
        } else {
            statements.push(hir::Statement {
                span: value.span,
                kind: hir::StatementKind::Expr(value),
            });
            None
        };
        ValueBlock { statements, value }
    }
}
