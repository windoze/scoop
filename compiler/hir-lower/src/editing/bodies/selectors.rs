use super::*;

#[derive(Clone)]
pub(super) struct SelectorInput {
    selector: ast::Span,
    safe_navigation: bool,
}

impl DeclarationAnalysis {
    pub fn selectors(
        &mut self,
        body: impl Into<hir::EditingSourceBodyId>,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&[hir::EditingSelector]>, EditingCancelled> {
        let body = body.into();
        self.source_body_facts(body, is_cancelled)?;
        Ok(match &self.body_facts[&body] {
            V::Known(body) => V::Known(&body.selectors),
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }

    pub fn selector_members(
        &mut self,
        body: impl Into<hir::EditingSourceBodyId>,
        selector: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<&hir::EditingMembers>>, EditingCancelled> {
        let body = body.into();
        let check = std::rc::Rc::new(is_cancelled);
        let cancellation = check.clone();
        self.source_body_facts(body, move || cancellation())?;
        let observed = match self.body_facts.get_mut(&body).expect("queried body") {
            V::Known(observed) => observed,
            V::Unavailable(reason) => return Ok(V::Unavailable(*reason)),
        };
        if !observed.selector_members.contains_key(&selector) {
            let Some(input) = observed
                .selectors
                .iter()
                .find(|input| input.selector == selector)
            else {
                return Ok(V::Known(None));
            };
            let members = match &input.receiver_types {
                V::Unavailable(reason) => V::Unavailable(*reason),
                V::Known(types) => {
                    let context = &observed.contexts[&input.context];
                    let mut candidate = self.lowerer.clone();
                    let members = candidate.with_type_query_context(
                        context.file,
                        context.owner,
                        context.parameters.clone(),
                        Some(input.context),
                        |lowerer| lowerer.editing_type_members(types, &|| check()),
                    )?;
                    if check() {
                        return Err(EditingCancelled);
                    }
                    self.lowerer = candidate;
                    members
                }
            };
            observed.selector_members.insert(selector, members);
        }
        Ok(match &observed.selector_members[&selector] {
            V::Known(members) => V::Known(Some(members)),
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
            .and_then(|(binding, _, _)| self.editing_local_definitions.get(binding))
            .map(|definition| definition.reference.clone());
        for input in inputs {
            let receiver_types = self.editing_receiver_types(value, input.safe_navigation);
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
                        receiver_types,
                    },
                );
        }
    }
}
