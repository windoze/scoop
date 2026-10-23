use super::*;
use crate::globals::{PendingRuntimeInitializer, PendingRuntimeInitializerKind};
use crate::properties::{PropertyAccessorBodyKind, PropertyAccessorKind, PropertyAccessorSource};
use hir::EditingValue as V;

mod availability;
mod parameters;

pub(super) enum SourceBodyInput<'a> {
    Function(&'a ast::FunctionDecl),
    Accessor(&'a PropertyAccessorSource),
    Initializer(&'a PendingRuntimeInitializer),
}

impl DeclarationAnalysis {
    pub fn source_body_at(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<Option<hir::EditingSourceBody>, EditingCancelled> {
        let check = std::rc::Rc::new(is_cancelled);
        let mut previous = None;
        loop {
            if check() {
                return Err(EditingCancelled);
            }
            let regions = self.source_body_regions().collect::<Vec<_>>();
            let selected = regions
                .iter()
                .filter(|body| {
                    &body.origin.source == source
                        && body.origin.span.start <= span.start
                        && span.end <= body.origin.span.end
                })
                .filter(|body| self.source_body_scope_available(&body.origin, &regions))
                .min_by_key(|body| body.origin.span.end - body.origin.span.start)
                .cloned();
            let Some(body) = selected else {
                return Ok(None);
            };
            if previous == Some(body.id) {
                return Ok(Some(body));
            }
            previous = Some(body.id);
            let cancellation = check.clone();
            self.source_body_facts(body.id, move || cancellation())?;
        }
    }

    pub fn source_bodies(&self) -> Vec<hir::EditingSourceBody> {
        let regions = self.source_body_regions().collect::<Vec<_>>();
        regions
            .iter()
            .filter(|body| self.source_body_scope_available(&body.origin, &regions))
            .cloned()
            .collect()
    }

    pub fn source_body_signature(
        &self,
        body: &hir::EditingSourceBody,
    ) -> Option<V<EditingSignature>> {
        let function = match body.id {
            hir::EditingSourceBodyId::Function(function) if body.callable().is_some() => function,
            hir::EditingSourceBodyId::Default(hir::SourceDefaultKey {
                owner: hir::SourceParameterOwner::Function(function),
                ..
            }) => function,
            _ => return None,
        };
        Some(self.signature(function))
    }

    fn source_body_regions(&self) -> impl Iterator<Item = hir::EditingSourceBody> + '_ {
        self.functions
            .iter()
            .filter_map(|function| {
                Some(hir::EditingSourceBody {
                    id: function.function.into(),
                    owner: hir::EditingSourceBodyOwner::Callable(hir::SemanticCallable::Current(
                        function.function,
                    )),
                    origin: hir::SemanticSourceLocation {
                        source: function.name_origin.source.clone(),
                        span: self.function_body_span(function)?,
                    },
                })
            })
            .chain(
                self.lowerer
                    .property_accessor_sources
                    .iter()
                    .filter_map(|source| {
                        if !has_source_body(source) {
                            return None;
                        }
                        Some(hir::EditingSourceBody {
                            id: source.function.into(),
                            owner: hir::EditingSourceBodyOwner::Callable(
                                self.lowerer.accessor_callable(source),
                            ),
                            origin: self.lowerer.accessor_source_origin(source),
                        })
                    }),
            )
            .chain(
                self.lowerer
                    .pending_runtime_initializers
                    .iter()
                    .map(|source| hir::EditingSourceBody {
                        id: source.function.into(),
                        owner: hir::EditingSourceBodyOwner::Initialization(source.unit),
                        origin: self.lowerer.initializer_source_origin(source),
                    }),
            )
            .chain(self.lowerer.editing_default_sources())
            .chain(self.lowerer.editing_construction_sources())
    }

    fn function_body_span(&self, function: &hir::EditingFunction) -> Option<ast::Span> {
        if let Some(declaration) = self.body_inputs.get(&function.function) {
            return match &declaration.body {
                ast::FunctionBody::Block(block) => Some(block.span),
                ast::FunctionBody::Expr(expression) => Some(expression.span()),
                ast::FunctionBody::None => None,
            };
        }
        let body = self.recovered_bodies.get(&function.function)?;
        let start = match body.root {
            ast::RecoveredBodyRoot::Block(ast::BodyBlockId(start))
            | ast::RecoveredBodyRoot::Expression(start) => start,
        };
        Some(ast::Span::new(start, function.span.end))
    }

    pub(super) fn source_body_input(
        &self,
        function: hir::FunctionId,
    ) -> Option<SourceBodyInput<'_>> {
        if let Some(declaration) = self.body_inputs.get(&function) {
            return Some(SourceBodyInput::Function(declaration));
        }
        if let Some(source) = self.lowerer.editing_accessor_source(function) {
            return self
                .declaration_scope_available(&self.lowerer.accessor_source_origin(source))
                .then_some(SourceBodyInput::Accessor(source));
        }
        let source = self
            .lowerer
            .pending_runtime_initializers
            .iter()
            .find(|source| source.function == function)?;
        self.declaration_scope_available(&self.lowerer.initializer_source_origin(source))
            .then_some(SourceBodyInput::Initializer(source))
    }
}

impl Lowerer {
    pub(super) fn editing_accessor_source(
        &self,
        function: hir::FunctionId,
    ) -> Option<&PropertyAccessorSource> {
        self.property_accessor_sources
            .iter()
            .find(|source| source.function == function && has_source_body(source))
    }

    pub(super) fn accessor_callable(
        &self,
        source: &PropertyAccessorSource,
    ) -> hir::SemanticCallable {
        let property = &self.properties[source.property];
        match source.kind {
            PropertyAccessorKind::Getter => {
                hir::SemanticCallable::CurrentGetter(property.capability.getter())
            }
            PropertyAccessorKind::Setter => hir::SemanticCallable::CurrentSetter(
                property
                    .capability
                    .setter()
                    .expect("a setter source belongs to a writable property"),
            ),
        }
    }

    fn accessor_source_origin(
        &self,
        source: &PropertyAccessorSource,
    ) -> hir::SemanticSourceLocation {
        hir::SemanticSourceLocation {
            source: self
                .source_identity(self.function_files[&source.function])
                .clone(),
            span: source.declaration.span,
        }
    }

    fn initializer_source_origin(
        &self,
        source: &PendingRuntimeInitializer,
    ) -> hir::SemanticSourceLocation {
        let expression = match &source.kind {
            PendingRuntimeInitializerKind::Stored { expression, .. }
            | PendingRuntimeInitializerKind::Delegated { expression, .. } => expression,
        };
        hir::SemanticSourceLocation {
            source: self.source_identity(source.file).clone(),
            span: expression.span(),
        }
    }

    pub(super) fn lower_editing_initializer(&mut self, source: &PendingRuntimeInitializer) -> bool {
        let property = match source.kind {
            PendingRuntimeInitializerKind::Stored { storage, .. } => self.globals[storage].property,
            PendingRuntimeInitializerKind::Delegated { property, .. } => property,
        };
        if self.property_type_pending(property) {
            self.infer_property_type(property).is_some()
        } else {
            self.lower_runtime_initializer(source).is_some()
        }
    }
}

fn has_source_body(source: &PropertyAccessorSource) -> bool {
    source.body_kind == PropertyAccessorBodyKind::Source
        && !matches!(source.declaration.body, ast::FunctionBody::None)
}
