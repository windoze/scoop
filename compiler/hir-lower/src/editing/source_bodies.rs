use super::*;
use crate::globals::{PendingRuntimeInitializer, PendingRuntimeInitializerKind};
use crate::properties::{PropertyAccessorBodyKind, PropertyAccessorKind, PropertyAccessorSource};
use hir::EditingValue as V;

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
            let selected = self
                .source_bodies()
                .into_iter()
                .filter(|body| {
                    &body.origin.source == source
                        && body.origin.span.start <= span.start
                        && span.end <= body.origin.span.end
                })
                .min_by_key(|body| body.origin.span.end - body.origin.span.start);
            let Some(body) = selected else {
                return Ok(None);
            };
            if previous == Some(body.function) {
                return Ok(Some(body));
            }
            previous = Some(body.function);
            let cancellation = check.clone();
            self.function_facts(body.function, move || cancellation())?;
        }
    }

    pub fn source_bodies(&self) -> Vec<hir::EditingSourceBody> {
        self.source_body_regions()
            .filter(|body| self.declaration_scope_available(&body.origin))
            .collect()
    }

    pub fn source_body_signature(
        &self,
        body: &hir::EditingSourceBody,
    ) -> Option<V<EditingSignature>> {
        body.callable().map(|_| self.signature(body.function))
    }

    fn source_body_regions(&self) -> impl Iterator<Item = hir::EditingSourceBody> + '_ {
        self.functions
            .iter()
            .map(|function| hir::EditingSourceBody {
                function: function.function,
                owner: hir::EditingSourceBodyOwner::Callable(hir::SemanticCallable::Current(
                    function.function,
                )),
                origin: hir::SemanticSourceLocation {
                    source: function.name_origin.source.clone(),
                    span: function.span,
                },
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
                            function: source.function,
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
                        function: source.function,
                        owner: hir::EditingSourceBodyOwner::Initialization(source.unit),
                        origin: self.lowerer.initializer_source_origin(source),
                    }),
            )
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

impl DeclarationAnalysis {
    pub(super) fn declaration_scope_available(&self, origin: &hir::SemanticSourceLocation) -> bool {
        self.source_body_regions()
            .filter(|body| {
                body.origin.source == origin.source
                    && body.origin.span != origin.span
                    && body.origin.span.start <= origin.span.start
                    && origin.span.end <= body.origin.span.end
                    && self.lowerer.editing_accessor_source(body.function)
                        .is_none_or(|source| self.lowerer.property_name_spans.get(&source.property) != Some(&origin.span))
            })
            .all(|body| matches!(self.body_facts.get(&body.function), Some(V::Known(body)) if body.available()))
    }
}
