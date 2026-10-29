use super::source_bodies::SourceBodyInput;
use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

mod calls;
mod imports;
mod observations;
mod operators;
mod prepared;
mod properties;
mod record;
mod rejected_references;
mod selectors;
pub(in crate::editing) use calls::CallInput;
pub(in crate::editing) use observations::ObservedContext;
pub(crate) use observations::{BodyObservation, ObservedBody};

impl DeclarationAnalysis {
    pub fn describe_call_type(
        &mut self,
        context: &hir::EditingCallContext,
        ty: hir::TypeId,
    ) -> Result<V<hir::SemanticTypeDescription>, ast::Diagnostic> {
        match context {
            hir::EditingCallContext::Body { body, source } => {
                self.describe_body_type(*body, *source, ty)
            }
            hir::EditingCallContext::Declaration(binders) => {
                self.describe_declaration_type(binders, ty).map(V::Known)
            }
        }
    }

    pub fn function_facts(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&hir::EditingBodyFacts>, EditingCancelled> {
        self.source_body_facts(function.into(), is_cancelled)
    }

    pub fn source_body_facts(
        &mut self,
        body: hir::EditingSourceBodyId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&hir::EditingBodyFacts>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.body_facts.contains_key(&body) {
            let facts = match body {
                hir::EditingSourceBodyId::Function(function) => {
                    self.analyze_function_body(function, is_cancelled)?
                }
                hir::EditingSourceBodyId::Default(key) => {
                    self.analyze_default_body(key, is_cancelled)?
                }
                hir::EditingSourceBodyId::Construction(body) => {
                    self.analyze_construction_body(body, is_cancelled)?
                }
            };
            self.body_facts.insert(body, facts);
        }
        Ok(match &self.body_facts[&body] {
            V::Known(body) => match &body.facts {
                V::Known(facts) => V::Known(facts),
                V::Unavailable(reason) => V::Unavailable(*reason),
            },
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }

    fn analyze_function_body(
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
            function.into(),
            hir::LexicalDefinitionRoot::Function(function),
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
        let cancellation = observed.cancellation();
        let facts = V::Known(observed.finish(&mut candidate, completed));
        if cancellation() {
            return Err(EditingCancelled);
        }
        self.lowerer = candidate;
        Ok(facts)
    }

    fn analyze_default_body(
        &mut self,
        key: hir::SourceDefaultKey,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<ObservedBody>, EditingCancelled> {
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if let hir::SourceParameterOwner::Function(function) = key.owner
            && let V::Unavailable(reason) = self.signature(function)
        {
            return Ok(V::Unavailable(reason));
        }
        let Some(source) = self.lowerer.editing_default_source(key) else {
            return Ok(V::Unavailable(U::IncompleteSyntax));
        };
        if !self.declaration_scope_available(&source.origin) {
            return Ok(V::Unavailable(U::BodyFailure));
        }
        let mut candidate = self.lowerer.clone();
        let cancellation = crate::pipeline::cancellation::FrontendCancellation::new(is_cancelled);
        let previous = candidate.cancellation.replace(cancellation.clone());
        let facts = candidate.lower_editing_default(key);
        if cancellation.cancelled() {
            return Err(EditingCancelled);
        }
        candidate.cancellation = previous;
        self.lowerer = candidate;
        Ok(facts.map_or(V::Unavailable(U::BodyFailure), V::Known))
    }

    pub fn describe_body_type(
        &mut self,
        body: impl Into<hir::EditingSourceBodyId>,
        context: hir::SourceContextId,
        ty: hir::TypeId,
    ) -> Result<V<hir::SemanticTypeDescription>, ast::Diagnostic> {
        let body = body.into();
        let context_data = self
            .body_facts
            .get(&body)
            .and_then(|body| {
                let V::Known(body) = body else { return None };
                body.contexts.get(&context)
            })
            .or_else(|| {
                self.recovered_facts
                    .get(&body.function()?)?
                    .contexts
                    .get(&context)
            })
            .cloned();
        let Some(context_data) = context_data else {
            return Ok(V::Unavailable(U::BodyFailure));
        };
        self.lowerer.with_type_query_context(
            context_data.file,
            context_data.owner,
            context_data.parameters,
            Some(context),
            |lowerer| lowerer.describe_semantic_type(ty).map(V::Known),
        )
    }
}
