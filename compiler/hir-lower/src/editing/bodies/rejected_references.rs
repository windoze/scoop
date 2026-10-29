//! A resolved declaration remains navigable when its reference is forbidden.

use super::*;

impl DeclarationAnalysis {
    pub fn callable_references(
        &mut self,
        body: impl Into<hir::EditingSourceBodyId>,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&[hir::EditingCallableReference]>, EditingCancelled> {
        let body = body.into();
        self.source_body_facts(body, is_cancelled)?;
        Ok(match &self.body_facts[&body] {
            V::Known(body) => V::Known(match &body.facts {
                V::Known(facts) => &facts.callables,
                V::Unavailable(_) => &body.rejected_callables,
            }),
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }
}

impl Lowerer {
    pub(crate) fn observe_rejected_callable_reference(
        &mut self,
        span: ast::Span,
        target: hir::SemanticCallable,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active query");
        let Some(&span) = body.callable_names.get(&span) else {
            return;
        };
        let reference = hir::EditingCallableReference {
            span,
            target: target.into(),
            definition: self.editing_callee_origin(target.into()),
        };
        self.editing_body
            .as_mut()
            .expect("active query")
            .rejected_callables
            .insert(span, reference);
    }

    pub(crate) fn retain_rejected_callable_references(&mut self, failed: &Lowerer) {
        if let (Some(body), Some(failed)) = (&mut self.editing_body, &failed.editing_body) {
            body.rejected_callables
                .extend(failed.rejected_callables.clone());
        }
    }
}
