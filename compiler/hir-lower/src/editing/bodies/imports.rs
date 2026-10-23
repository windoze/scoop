use super::*;
use crate::imports::ImportLookupLayer;

impl Lowerer {
    pub(crate) fn observe_operator_import_lookup(
        &mut self,
        span: ast::Span,
        layer: ImportLookupLayer,
    ) {
        if layer != ImportLookupLayer::Exact || !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active body query");
        let Some(&span) = body.callable_names.get(&span) else {
            return;
        };
        let Some(callable) = body
            .callables
            .get(&span)
            .and_then(|reference| reference.target.callable())
        else {
            return;
        };
        let imports = self.imports.files[&self.current_file]
            .exact
            .iter()
            .filter(|import| import.targets.iter().any(|binding| {
                matches!(self.editing_import_member(binding), Some(hir::EditingMemberTarget::Callable(target)) if target == callable)
            }))
            .map(|import| hir::SemanticSourceLocation {
                source: import.origin.source.clone(),
                span: import.origin.span,
            })
            .collect();
        self.editing_body
            .as_mut()
            .expect("active body query")
            .imports
            .insert(span, imports);
    }

    /// Called only after ordinary lookup selected this source-level layer.
    pub(crate) fn observe_import_lookup(&mut self, name: &ast::Ident, layer: ImportLookupLayer) {
        self.observe_constant_import(name, layer);
        if layer != ImportLookupLayer::Exact || !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active body query");
        let Some(span) = body
            .callable_names
            .get(&name.span)
            .or_else(|| body.property_names.get(&name.span))
            .copied()
        else {
            return;
        };
        let imports = self.editing_exact_import_origins(name);
        self.editing_body
            .as_mut()
            .expect("active body query")
            .imports
            .insert(span, imports);
    }

    pub(crate) fn editing_exact_import_origins(
        &self,
        name: &ast::Ident,
    ) -> Vec<hir::SemanticSourceLocation> {
        self.imports.files[&self.current_file]
            .exact
            .iter()
            .filter(|import| import.local_name == name.text)
            .map(|import| hir::SemanticSourceLocation {
                source: import.origin.source.clone(),
                span: import.origin.span,
            })
            .collect()
    }
}
