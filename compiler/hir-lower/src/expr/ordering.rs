use super::*;

impl Lowerer {
    /// Finish the earlier operand before statements produced by the later one.
    pub(crate) fn lower_ordered_rhs(
        &mut self,
        lhs: hir::Expr,
        rhs: &ast::Expr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        label: &str,
    ) -> Option<(hir::Expr, hir::Expr)> {
        let mut rhs_sink = Vec::new();
        let rhs = self.lower_expr(rhs, &mut rhs_sink, expected)?;
        let lhs = if rhs_sink.is_empty() {
            lhs
        } else {
            let span = lhs.span;
            self.materialize_temporary(label.into(), lhs, span, sink)
        };
        sink.extend(rhs_sink);
        Some((lhs, rhs))
    }
}
