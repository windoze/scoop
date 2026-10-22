use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

mod calls;
mod observations;
mod record;
mod selectors;
pub(crate) use observations::{BodyObservation, ObservedBody};

impl DeclarationAnalysis {
    pub fn function_facts(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&hir::EditingBodyFacts>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.body_facts.contains_key(&function) {
            let facts = match self.signature(function) {
                V::Unavailable(reason) => V::Unavailable(reason),
                V::Known(_) => match self.body_inputs.get(&function) {
                    Some(declaration) => {
                        let mut candidate = self.lowerer.clone();
                        let before = candidate.diagnostics.len();
                        candidate.current_file = candidate
                            .function_definition_file(function)
                            .expect("source function has its definition file");
                        candidate.editing_body = Some(BodyObservation::new(
                            function,
                            candidate.current_file,
                            before,
                            is_cancelled,
                        ));
                        let completed = if matches!(declaration.body, ast::FunctionBody::None) {
                            true
                        } else if candidate.signatures[&function].result_pending {
                            candidate.infer_function_result(function).is_some()
                        } else {
                            let body = candidate.lower_body(function, declaration);
                            candidate.functions[function].kind = hir::FunctionKind::User(body);
                            true
                        };
                        let observed = candidate.editing_body.take().expect("active body query");
                        if observed.cancelled() {
                            return Err(EditingCancelled);
                        }
                        let complete =
                            completed && !observed.failed && candidate.diagnostics.len() == before;
                        let facts = V::Known(observed.finish(&candidate, complete));
                        self.lowerer = candidate;
                        facts
                    }
                    None => V::Unavailable(U::IncompleteSyntax),
                },
            };
            self.body_facts.insert(function, facts);
        }
        Ok(match &self.body_facts[&function] {
            V::Known(body) => match &body.facts {
                V::Known(facts) => V::Known(facts),
                V::Unavailable(reason) => V::Unavailable(*reason),
            },
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }

    pub fn describe_body_type(
        &mut self,
        function: hir::FunctionId,
        context: hir::SourceContextId,
        ty: hir::TypeId,
    ) -> Result<V<hir::SemanticTypeDescription>, ast::Diagnostic> {
        let Some(V::Known(body)) = self.body_facts.get(&function) else {
            return Ok(V::Unavailable(U::BodyFailure));
        };
        let Some(context_data) = body.contexts.get(&context).cloned() else {
            return Ok(V::Unavailable(U::BodyFailure));
        };
        self.lowerer
            .in_context(hir::SemanticContext::Function(function), |lowerer| {
                lowerer.current_owner = context_data.owner;
                lowerer.current_source_context = Some(context);
                lowerer.type_params_in_scope = context_data.parameters;
                lowerer.describe_semantic_type(ty).map(V::Known)
            })
    }
}
