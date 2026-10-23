use super::*;
use crate::properties::{PropertyAccessorBodyKind, PropertyAccessorKind, PropertyAccessorSource};
use hir::EditingValue as V;

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
        self.functions
            .iter()
            .map(|function| hir::EditingSourceBody {
                function: function.function,
                callable: hir::SemanticCallable::Current(function.function),
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
                        let origin = self.lowerer.accessor_source_origin(source);
                        self.declaration_scope_available(&origin)
                            .then(|| hir::EditingSourceBody {
                                function: source.function,
                                callable: self.lowerer.accessor_callable(source),
                                origin,
                            })
                    }),
            )
            .collect()
    }

    pub(super) fn source_body_declaration(
        &self,
        function: hir::FunctionId,
    ) -> Option<&ast::FunctionDecl> {
        self.body_inputs.get(&function).or_else(|| {
            let source = self.lowerer.editing_accessor_source(function)?;
            self.declaration_scope_available(&self.lowerer.accessor_source_origin(source))
                .then_some(&source.declaration)
        })
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
}

fn has_source_body(source: &PropertyAccessorSource) -> bool {
    source.body_kind == PropertyAccessorBodyKind::Source
        && !matches!(source.declaration.body, ast::FunctionBody::None)
}

impl DeclarationAnalysis {
    pub(super) fn declaration_scope_available(&self, origin: &hir::SemanticSourceLocation) -> bool {
        let functions = self
            .functions
            .iter()
            .filter(|function| {
                function.name_origin.source == origin.source
                    && function.span.start <= origin.span.start
                    && origin.span.end <= function.span.end
            })
            .map(|function| function.function);
        let accessors = self
            .lowerer
            .property_accessor_sources
            .iter()
            .filter(|source| {
                has_source_body(source)
                    && *self
                        .lowerer
                        .source_identity(self.lowerer.function_files[&source.function])
                        == origin.source
                    && source.declaration.span != origin.span
                    && source.declaration.span.start <= origin.span.start
                    && origin.span.end <= source.declaration.span.end
                    && self.lowerer.property_name_spans.get(&source.property) != Some(&origin.span)
            })
            .map(|source| source.function);
        functions.chain(accessors).all(|function| matches!(self.body_facts.get(&function), Some(V::Known(body)) if body.available()))
    }
}
