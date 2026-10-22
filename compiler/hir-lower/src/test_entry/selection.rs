use super::*;

impl Lowerer {
    pub(super) fn test_case_selected(
        &mut self,
        arguments: hir::Expr,
        index: usize,
        case: &hir::TestCase,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let span = case.method.origin.span;
        let key = self.test_case_key(index, span);
        let ignored = hir::Expr {
            kind: hir::ExprKind::BoolLiteral(case.ignore_reason.is_some()),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        };
        let name = hir::Expr {
            kind: hir::ExprKind::StringLiteral {
                value: case.display_name.clone(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            ty: self.string,
            span,
            origin: self.expression_origin(span),
        };
        let selected = self.test_api_call(
            "isTestSelected",
            vec![arguments, key, ignored, name],
            span,
            sink,
        )?;
        if !self.types_equal(selected.ty, self.boolean) {
            self.error(
                span,
                "test execution requires isTestSelected to return Boolean".into(),
            );
            return None;
        }
        Some(selected)
    }

    pub(super) fn test_case_key(&self, index: usize, span: ast::Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::StringLiteral {
                value: index.to_string(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            ty: self.string,
            span,
            origin: self.expression_origin(span),
        }
    }
}
