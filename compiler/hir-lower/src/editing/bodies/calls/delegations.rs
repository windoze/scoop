use super::*;

impl Lowerer {
    pub(crate) fn observe_constructor_delegation(
        &mut self,
        constructor: hir::SourceContextConstructor,
        span: ast::Span,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let Some(input) = self.constructor_delegation_input(constructor, span) else {
            return;
        };
        let body = self
            .editing_body
            .as_mut()
            .expect("active construction query");
        body.delegation_name = input.callee;
        if let Some(callee) = input.callee {
            body.callable_names.insert(callee, callee);
            body.callable_names.insert(span, callee);
        }
        if !body.failed && self.diagnostics.len() == body.diagnostics_start {
            body.call_inputs.insert(span, input);
        }
    }

    pub(crate) fn observe_completed_delegation(&mut self, span: ast::Span, ty: hir::TypeId) {
        if !self.observes_current_body() {
            return;
        }
        let context = self.observe_body_context();
        self.observe_completed_call(span, ty, context);
    }
}
