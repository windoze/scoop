use super::*;
use crate::imports::ImportLookupLayer;

impl Lowerer {
    /// Called only after ordinary lookup selected this source-level layer.
    pub(crate) fn observe_import_lookup(&mut self, name: &ast::Ident, layer: ImportLookupLayer) {
        if layer != ImportLookupLayer::Exact || !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active body query");
        if !body.callable_names.contains_key(&name.span)
            && !body.property_names.contains_key(&name.span)
        {
            return;
        }
        let imports = self.imports.files[&self.current_file]
            .exact
            .iter()
            .filter(|import| import.local_name == name.text)
            .map(|import| hir::SemanticSourceLocation {
                source: import.origin.source.clone(),
                span: import.origin.span,
            })
            .collect();
        self.editing_body
            .as_mut()
            .expect("active body query")
            .imports
            .insert(name.span, imports);
    }
}
