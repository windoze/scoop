use super::*;

impl Lowerer {
    pub(crate) fn observe_call_syntax(&mut self, expression: &ast::Expr) {
        if !self.observes_current_body() {
            return;
        }
        let name = match expression {
            ast::Expr::Call(call) => &call.callee,
            ast::Expr::MethodCall { name, .. }
            | ast::Expr::SuperMethodCall { name, .. }
            | ast::Expr::QualifiedInterfaceSuperMethodCall { name, .. } => name,
            _ => return,
        };
        self.editing_body
            .as_mut()
            .expect("active query")
            .call_names
            .insert(name.span);
    }

    pub(crate) fn observe_callable_reference(
        &mut self,
        span: ast::Span,
        target: hir::SemanticCallable,
    ) {
        if !self.observes_current_body()
            || !self
                .editing_body
                .as_ref()
                .expect("active query")
                .call_names
                .contains(&span)
        {
            return;
        }
        let definition = self.editing_callable_origin(target);
        self.editing_body
            .as_mut()
            .expect("active query")
            .callables
            .insert(
                span,
                hir::EditingCallableReference {
                    span,
                    target,
                    definition,
                },
            );
    }
}
