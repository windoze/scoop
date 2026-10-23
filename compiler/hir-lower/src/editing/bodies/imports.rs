use super::*;
use crate::imports::ImportLookupLayer;

mod operators;

impl Lowerer {
    pub(crate) fn observe_operator_import_lookup(
        &mut self,
        span: ast::Span,
        layer: ImportLookupLayer,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active body query");
        let Some(&span) = body.callable_names.get(&span) else {
            return;
        };
        let Some(callable) = body
            .callables
            .get(&span)
            .and_then(|references| references.last())
            .and_then(|reference| reference.target.callable())
        else {
            return;
        };
        let imports = self.editing_operator_import_sources(callable, layer);
        self.observe_selected_imports(span, imports);
    }

    /// Called only after ordinary lookup selected this source-level layer.
    pub(crate) fn observe_import_lookup(&mut self, name: &ast::Ident, layer: ImportLookupLayer) {
        self.observe_constant_import(name, layer);
        if !self.observes_current_body() {
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
        let imports = self.editing_import_sources(name, layer);
        self.observe_selected_imports(span, imports);
    }

    fn observe_selected_imports(
        &mut self,
        span: ast::Span,
        imports: Vec<hir::EditingImportSource>,
    ) {
        let selected = self
            .editing_body
            .as_mut()
            .expect("active body query")
            .imports
            .entry(span)
            .or_default();
        for origin in imports {
            if !selected.contains(&origin) {
                selected.push(origin);
            }
        }
    }

    pub(crate) fn editing_import_sources(
        &self,
        name: &ast::Ident,
        layer: ImportLookupLayer,
    ) -> Vec<hir::EditingImportSource> {
        let file = &self.imports.files[&self.current_file];
        let declaration = |origin: &crate::imports::ImportSyntaxOrigin| {
            hir::EditingImportSource::Declaration(hir::SemanticSourceLocation {
                source: origin.source.clone(),
                span: origin.span,
            })
        };
        match layer {
            ImportLookupLayer::Exact => file
                .exact
                .iter()
                .filter(|import| import.local_name == name.text)
                .map(|import| declaration(&import.origin))
                .collect(),
            ImportLookupLayer::Star => file
                .stars
                .iter()
                .flat_map(|import| {
                    import
                        .snapshot
                        .get(&name.text)
                        .into_iter()
                        .flat_map(|targets| targets.iter())
                        .filter_map(|target| target.direct_binding())
                        .map(|binding| hir::EditingImportSource::Dependency {
                            import: Some(hir::SemanticSourceLocation {
                                source: import.origin.source.clone(),
                                span: import.origin.span,
                            }),
                            target: binding.target(),
                            routes: binding
                                .sources()
                                .map(|source| source.route().clone())
                                .collect(),
                        })
                })
                .collect(),
            ImportLookupLayer::CurrentPackage(_) => file
                .current_package_dependencies
                .get(&name.text)
                .into_iter()
                .flatten()
                .map(Self::editing_dependency_import)
                .collect(),
            ImportLookupLayer::CorePrelude | ImportLookupLayer::QualifiedPackage => Vec::new(),
        }
    }

    pub(crate) fn editing_dependency_import(
        binding: &hir::DirectImportedTargetBinding,
    ) -> hir::EditingImportSource {
        hir::EditingImportSource::Dependency {
            import: None,
            target: binding.target(),
            routes: binding
                .sources()
                .map(|source| source.route().clone())
                .collect(),
        }
    }
}
