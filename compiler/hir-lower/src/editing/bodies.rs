use super::source_bodies::SourceBodyInput;
use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

mod calls;
mod imports;
mod observations;
mod properties;
mod record;
mod selectors;
pub(in crate::editing) use observations::ObservedContext;
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
            let facts = self.analyze_source_body(function, is_cancelled)?;
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

    fn analyze_source_body(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<ObservedBody>, EditingCancelled> {
        let input = self.source_body_input(function);
        if !matches!(input, Some(SourceBodyInput::Initializer(_)))
            && let V::Unavailable(reason) = self.signature(function)
            && !(reason == U::PendingInference
                && self.lowerer.editing_accessor_source(function).is_some())
        {
            return Ok(V::Unavailable(reason));
        }
        let Some(input) = input else {
            return Ok(V::Unavailable(U::IncompleteSyntax));
        };
        let mut candidate = self.lowerer.clone();
        let before = candidate.diagnostics.len();
        candidate.current_file = candidate
            .function_definition_file(function)
            .expect("source body has its definition file");
        candidate.editing_body = Some(BodyObservation::new(
            function,
            candidate.current_file,
            before,
            is_cancelled,
        ));
        let completed = match input {
            SourceBodyInput::Accessor(source) => {
                let pending = candidate.property_type_pending(source.property);
                if candidate.infer_property_type(source.property).is_none() {
                    false
                } else {
                    if !pending || !candidate.property_accessor_was_lowered(function) {
                        candidate.lower_property_accessor_body(source);
                    }
                    true
                }
            }
            SourceBodyInput::Initializer(source) => candidate.lower_editing_initializer(source),
            SourceBodyInput::Function(declaration)
                if matches!(declaration.body, ast::FunctionBody::None) =>
            {
                true
            }
            SourceBodyInput::Function(_) if candidate.signatures[&function].result_pending => {
                candidate.infer_function_result(function).is_some()
            }
            SourceBodyInput::Function(declaration) => {
                let body = candidate.lower_body(function, declaration);
                candidate.functions[function].kind = hir::FunctionKind::User(body);
                true
            }
        };
        let observed = candidate.editing_body.take().expect("active body query");
        if observed.cancelled() {
            return Err(EditingCancelled);
        }
        let complete = completed && !observed.failed && candidate.diagnostics.len() == before;
        let cancellation = observed.cancellation();
        let facts = V::Known(observed.finish(&mut candidate, complete));
        if cancellation() {
            return Err(EditingCancelled);
        }
        self.lowerer = candidate;
        Ok(facts)
    }

    pub fn describe_body_type(
        &mut self,
        function: hir::FunctionId,
        context: hir::SourceContextId,
        ty: hir::TypeId,
    ) -> Result<V<hir::SemanticTypeDescription>, ast::Diagnostic> {
        let context_data = self
            .body_facts
            .get(&function)
            .and_then(|body| {
                let V::Known(body) = body else { return None };
                body.contexts.get(&context)
            })
            .or_else(|| self.recovered_facts.get(&function)?.contexts.get(&context))
            .cloned();
        let Some(context_data) = context_data else {
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
