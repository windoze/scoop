use super::*;

#[derive(Clone)]
pub(super) struct SelectorInput {
    selector: ast::Span,
    safe_navigation: bool,
}

impl DeclarationAnalysis {
    pub fn selectors(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&[hir::EditingSelector]>, EditingCancelled> {
        self.function_facts(function, is_cancelled)?;
        Ok(match &self.body_facts[&function] {
            V::Known(body) => V::Known(&body.selectors),
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }
}

impl Lowerer {
    pub(crate) fn observe_selector_syntax(&mut self, expression: &ast::Expr) {
        if !self.observes_current_body() {
            return;
        }
        let (receiver, selector, navigation) = match expression {
            ast::Expr::MethodCall {
                receiver,
                name,
                navigation,
                ..
            } => (receiver.span(), name.span, *navigation),
            ast::Expr::FieldAccess(access) => {
                let selector = match &access.selector {
                    ast::FieldSelector::Name(name) => name.span,
                    ast::FieldSelector::Index(_, span) => *span,
                };
                (access.receiver.span(), selector, access.navigation)
            }
            _ => return,
        };
        let body = self.editing_body.as_mut().expect("active body query");
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return;
        }
        let selectors = body.selector_inputs.entry(receiver).or_default();
        if !selectors.iter().any(|input| input.selector == selector) {
            selectors.push(SelectorInput {
                selector,
                safe_navigation: navigation == ast::Navigation::Safe,
            });
        }
    }

    pub(super) fn observe_selector_receiver(
        &mut self,
        span: ast::Span,
        value: &hir::Expr,
        context: hir::SourceContextId,
    ) {
        let body = self.editing_body.as_ref().expect("active body query");
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return;
        }
        let Some(inputs) = body.selector_inputs.get(&span).cloned() else {
            return;
        };
        let local = body
            .uses
            .get(&span)
            .and_then(|(binding, _, _)| body.bindings.get(binding))
            .map(|definition| definition.reference.clone());
        let cancelled = body.cancellation();
        for input in inputs {
            let Ok(members) =
                self.editing_receiver_members(value, input.safe_navigation, &|| cancelled())
            else {
                return;
            };
            self.editing_body
                .as_mut()
                .expect("active body query")
                .selectors
                .insert(
                    input.selector,
                    hir::EditingSelector {
                        receiver: span,
                        selector: input.selector,
                        context,
                        value_type: value.ty,
                        local: local.clone(),
                        members,
                    },
                );
        }
    }
}
