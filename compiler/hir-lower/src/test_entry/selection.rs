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
        let key = hir::Expr {
            kind: hir::ExprKind::StringLiteral {
                value: index.to_string(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            ty: self.string,
            span,
            origin: self.expression_origin(span),
        };
        let ignored = hir::Expr {
            kind: hir::ExprKind::BoolLiteral(case.ignore_reason.is_some()),
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        };
        let selected =
            self.test_api_call("isTestSelected", vec![arguments, key, ignored], span, sink)?;
        if !self.types_equal(selected.ty, self.boolean) {
            self.error(
                span,
                "test execution requires isTestSelected to return Boolean".into(),
            );
            return None;
        }
        Some(selected)
    }
}
