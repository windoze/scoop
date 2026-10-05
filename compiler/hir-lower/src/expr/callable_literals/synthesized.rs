use super::*;

impl Lowerer {
    pub(crate) fn lower_synthesized_lambda(
        &mut self,
        parameter_type: TypeId,
        result_type: TypeId,
        span: Span,
        generate: impl FnOnce(&mut Self, hir::Expr) -> Option<crate::stmt::ValueBlock>,
    ) -> Option<hir::Expr> {
        let expected = self.intern_function_type(false, vec![parameter_type], result_type);
        self.with_pattern_transaction(|state| {
            state.lower_lambda_inner(false, None, span, span, Some(expected), |state, _| {
                let local = state
                    .scopes
                    .lookup("it")
                    .expect("the unary lambda parameter is bound");
                let parameter = hir::Expr {
                    kind: ExprKind::Local(local),
                    ty: parameter_type,
                    span,
                    origin: state.expression_origin(span),
                };
                generate(state, parameter)
            })
        })
    }
}
