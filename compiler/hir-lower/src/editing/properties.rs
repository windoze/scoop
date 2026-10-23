use super::*;
use hir::{EditingUnavailable as U, EditingValue as V, SemanticProperty as Property};

mod declarations;
mod imported;

impl DeclarationAnalysis {
    pub fn property_declaration(
        &mut self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<Option<hir::EditingPropertyDeclaration>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        let check = std::rc::Rc::new(is_cancelled);
        let cancellation = check.clone();
        if let Some(body) = self.source_body_at(source, span, move || cancellation())? {
            let body = body.id;
            let cancellation = check.clone();
            if let V::Unavailable(reason) = self.source_body_facts(body, move || cancellation())? {
                return Ok(V::Unavailable(reason));
            }
        }
        let mut selected =
            self.lowerer
                .property_name_spans
                .iter()
                .find_map(|(&property, &name)| {
                    if name != span {
                        return None;
                    }
                    let origin = self
                        .lowerer
                        .editing_property_origin(Property::Current(property))?;
                    (&origin.source == source).then_some(Property::Current(property))
                });
        if selected.is_none() {
            selected = self
                .constant_references
                .iter()
                .find(|reference| {
                    &reference.origin.source == source && reference.origin.span == span
                })
                .map(|reference| reference.target);
        }
        if selected.is_none() {
            for binding in self.import_bindings(source, span) {
                if check() {
                    return Err(EditingCancelled);
                }
                let property = match self.lowerer.editing_import_member(binding) {
                    Some(hir::EditingMemberTarget::Property(property)) => {
                        Property::Current(property)
                    }
                    Some(hir::EditingMemberTarget::ImportedProperty(property)) => {
                        Property::Imported(property)
                    }
                    _ => continue,
                };
                if selected.is_some_and(|previous| previous != property) {
                    return Ok(V::Known(None));
                }
                selected = Some(property);
            }
        }
        let Some(property) = selected else {
            return Ok(V::Known(None));
        };
        let result = match property {
            Property::Current(property) => self.lowerer.editing_property_declaration(property),
            Property::Imported(property) => {
                let binders = hir::SemanticDeclarationBinders {
                    owner: Vec::new(),
                    callable: Vec::new(),
                };
                self.lowerer.in_type_context((&binders).into(), |lowerer| {
                    lowerer
                        .editing_imported_property_declaration(property)
                        .map_or(V::Unavailable(U::SignatureFailure), V::Known)
                })
            }
        };
        if check() {
            return Err(EditingCancelled);
        }
        Ok(match result {
            V::Known(declaration) => V::Known(Some(declaration)),
            V::Unavailable(reason) => V::Unavailable(reason),
        })
    }
}
