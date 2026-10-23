use super::*;

impl DeclarationAnalysis {
    pub(in crate::editing) fn declaration_scope_available(
        &self,
        origin: &hir::SemanticSourceLocation,
    ) -> bool {
        self.lowerer.editing_declaration_scope_available(origin)
            && self
                .source_body_regions()
                .all(|body| self.enclosing_body_available(origin, &body))
    }

    pub(super) fn source_body_scope_available(
        &self,
        origin: &hir::SemanticSourceLocation,
        regions: &[hir::EditingSourceBody],
    ) -> bool {
        self.lowerer.editing_declaration_scope_available(origin)
            && regions
                .iter()
                .all(|body| self.enclosing_body_available(origin, body))
    }

    fn enclosing_body_available(
        &self,
        origin: &hir::SemanticSourceLocation,
        body: &hir::EditingSourceBody,
    ) -> bool {
        if body.origin.source != origin.source
            || body.origin.span == origin.span
            || origin.span.start < body.origin.span.start
            || body.origin.span.end < origin.span.end
        {
            return true;
        }
        if body
            .id
            .function()
            .and_then(|function| self.lowerer.editing_accessor_source(function))
            .is_some_and(|source| {
                self.lowerer.property_name_spans.get(&source.property) == Some(&origin.span)
            })
        {
            return true;
        }
        match self.body_facts.get(&body.id) {
            Some(V::Known(body)) => body.available(),
            Some(V::Unavailable(_)) => false,
            None => self
                .lowerer
                .editing_prepared_bodies
                .as_ref()
                .and_then(|facts| facts.get(&body.id))
                .is_some_and(|body| body.available()),
        }
    }
}
