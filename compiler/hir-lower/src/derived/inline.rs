use super::*;

impl Lowerer {
    pub(crate) fn lower_derived_equality_call(
        &mut self,
        application: hir::DerivedEqualityApplicationId,
        receiver: hir::Expr,
        other: hir::Expr,
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        if self.type_contains_param(receiver.ty) {
            let receiver =
                self.materialize_temporary("$equality.this".into(), receiver, span, sink);
            let other = self.materialize_temporary("$equality.other".into(), other, span, sink);
            let parameters = self.type_params_in_scope.clone();
            let bindings = parameters
                .into_iter()
                .map(|parameter| (parameter.id, self.intern_type(Type::Param(parameter.id))))
                .collect();
            return self.inline_derived_equality(application, receiver, other, bindings);
        }
        hir::Expr {
            kind: hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::DerivedEquality(application),
                args: vec![other],
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        }
    }
}
