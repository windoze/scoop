use super::*;

impl Lowerer {
    pub(super) fn decoding_long(&mut self, value: u64, span: Span) -> hir::Expr {
        let ty = self.intern_type(Type::Integer(hir::IntegerKind::SIGNED_64));
        self.coding_expr(
            hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed64(value)),
            ty,
            span,
        )
    }

    pub(super) fn decoding_branch(
        &mut self,
        condition: hir::Expr,
        ty: TypeId,
        mut yes: (Vec<hir::Statement>, hir::Expr),
        mut no: (Vec<hir::Statement>, hir::Expr),
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let local = self.alloc_hidden("decoded_branch", ty);
        yes.0.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: yes.1,
            },
            span,
        });
        no.0.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: no.1,
            },
            span,
        });
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: condition,
                then_body: yes.0,
                else_body: Some(no.0),
            },
            span,
        });
        self.coding_expr(hir::ExprKind::Local(local), ty, span)
    }

    pub(super) fn decoding_failure(
        &mut self,
        decoder: hir::Expr,
        message: &str,
        span: Span,
    ) -> Option<Vec<hir::Statement>> {
        let exception = self.core_coding_nominal("DecodingException")?;
        let ty = self
            .apply_nominal_type(exception, Vec::new())
            .expect("the core exception is a nominal type");
        let record = self.coding_primary(ty, span)?;
        let path = self.coding_property(decoder, "path", span)?;
        let message = self.coding_string(message, span);
        let mut statements = Vec::new();
        let value =
            self.call_coding_constructor(ty, &record, vec![path, message], span, &mut statements)?;
        statements.push(hir::Statement {
            kind: hir::StatementKind::Throw(value),
            span,
        });
        Some(statements)
    }
}
