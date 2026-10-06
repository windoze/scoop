use super::*;

impl Lowerer {
    pub(crate) fn lower_synthesized_lambda(
        &mut self,
        parameter_types: &[TypeId],
        result_type: TypeId,
        span: Span,
        generate: impl FnOnce(&mut Self, Vec<hir::Expr>) -> Option<crate::stmt::ValueBlock>,
    ) -> Option<hir::Expr> {
        let expected = self.intern_function_type(false, parameter_types.to_vec(), result_type);
        let names = (0..parameter_types.len())
            .map(|index| format!("__codec_argument_{index}"))
            .collect::<Vec<_>>();
        let parameters = names
            .iter()
            .map(|name| ast::LambdaParam {
                target: ast::Pattern::Binding(ast::Ident {
                    text: name.clone(),
                    span,
                }),
                ty: None,
                span,
            })
            .collect::<Vec<_>>();
        self.with_pattern_transaction(|state| {
            state.lower_lambda_inner(
                false,
                Some(&parameters),
                span,
                span,
                Some(expected),
                |state, _| {
                    let values = names
                        .iter()
                        .zip(parameter_types)
                        .map(|(name, &ty)| {
                            let local = state
                                .scopes
                                .lookup(name)
                                .expect("the synthesized lambda parameter is bound");
                            hir::Expr {
                                kind: ExprKind::Local(local),
                                ty,
                                span,
                                origin: state.expression_origin(span),
                            }
                        })
                        .collect();
                    generate(state, values)
                },
            )
        })
    }
}
