//! Cached projections borrow the ordinary graph, excluding query-only types.

use super::*;
use std::{cell::OnceCell, collections::HashMap, sync::Arc};

mod data;
mod defaults;
mod inputs;
mod retention;
pub(super) use data::ProjectionData;

pub(super) struct BodyQueries<'a> {
    lowerer: &'a Lowerer,
    data: OnceCell<Result<ProjectionData, Diagnostic>>,
    bodies: HashMap<hir::FunctionId, Arc<hir::ExportGenericCallableBodyV1>>,
    defaults: HashMap<hir::ExportDefaultSourceId, Arc<hir::ExportDefaultTemplateV1>>,
    imported_defaults: HashMap<hir::ExportDefaultTemplateKeyV1, Arc<hir::ExportDefaultTemplateV1>>,
}

impl<'a> BodyQueries<'a> {
    pub(super) fn new(lowerer: &'a Lowerer) -> Self {
        Self {
            lowerer,
            data: OnceCell::new(),
            bodies: HashMap::new(),
            defaults: HashMap::new(),
            imported_defaults: HashMap::new(),
        }
    }

    pub(super) fn data(&self) -> Result<&ProjectionData, Diagnostic> {
        self.data
            .get_or_init(|| ProjectionData::new(self.lowerer))
            .as_ref()
            .map_err(Clone::clone)
    }

    fn body(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<Arc<hir::ExportGenericCallableBodyV1>, Diagnostic> {
        if let Some(body) = self.bodies.get(&function) {
            return Ok(Arc::clone(body));
        }
        let lowerer = self.lowerer;
        let data = self.data()?;
        let file = lowerer
            .function_definition_file(function)
            .ok_or_else(|| projection_error("body has no bound definition file"))?;
        let source = &data.source_files[file];
        let context = lowerer.source_context_by_value
            [&hir::SourceContext::new(source.identity.clone(), hir::SourceContextSubject::File)];
        let origin = hir::DefinitionOrigin {
            provider: source.provider,
            file: file as u32,
            context,
            span: lowerer.functions[function].span,
        };
        let body = data
            .inputs(lowerer)
            .project_callable_body(function, origin, Some(&data.imported))
            .map_err(projection_error)?
            .expect("only an available ordinary body reaches projection");
        let body = Arc::new(body);
        self.bodies.insert(function, Arc::clone(&body));
        Ok(body)
    }
}

impl FrontendQueries<'_, '_> {
    pub(super) fn query_projected_body(
        &mut self,
        callable: hir::SemanticCallable,
    ) -> Result<
        Option<hir::SemanticBodyAvailability<Arc<hir::ExportGenericCallableBodyV1>>>,
        Diagnostic,
    > {
        use hir::SemanticBodyAvailability as A;
        let Some(body) = self.query_body(callable) else {
            return Ok(None);
        };
        Ok(Some(match body {
            A::PendingGeneration => A::PendingGeneration,
            A::NotPublished => A::NotPublished,
            A::NoBody => A::NoBody,
            A::Available(hir::SemanticBody::Imported(body)) => A::Available(body.shared_body()),
            A::Available(hir::SemanticBody::Current(_)) => {
                let function = match callable {
                    hir::SemanticCallable::Current(function) => function,
                    hir::SemanticCallable::CurrentGetter(getter) => {
                        accessor_function(self.lowerer.property_getters[getter].implementation)
                    }
                    hir::SemanticCallable::CurrentSetter(setter) => {
                        accessor_function(self.lowerer.property_setters[setter].implementation)
                    }
                    hir::SemanticCallable::Imported(_) => unreachable!("imported body is shared"),
                };
                A::Available(self.body_queries.body(function)?)
            }
        }))
    }
}

fn accessor_function(implementation: hir::PropertyAccessorImplementation) -> hir::FunctionId {
    match implementation {
        hir::PropertyAccessorImplementation::Body(function)
        | hir::PropertyAccessorImplementation::StorageBody(function) => function,
        hir::PropertyAccessorImplementation::Storage
        | hir::PropertyAccessorImplementation::Constant
        | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
            unreachable!("bodyless accessor was classified before projection")
        }
    }
}

fn projection_error(error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::at(
        Span::new(0, 0),
        format!("cannot project queried body: {error}"),
    )
}
