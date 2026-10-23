use super::*;
use crate::imports::ImportLookupLayer;

impl Lowerer {
    pub(super) fn observes_types(&self) -> bool {
        !self.in_semantic_query && (self.editing_types.is_some() || self.observes_current_body())
    }

    pub(crate) fn observe_type_lookup(
        &mut self,
        name: &ast::Ident,
        binding: &TypeLookupTarget,
        layer: Option<ImportLookupLayer>,
    ) {
        if !self.observes_types() {
            return;
        }
        let target = self.editing_lookup_type_target(binding);
        let mut imports =
            layer.map_or_else(Vec::new, |layer| self.editing_import_sources(name, layer));
        if let TypeLookupTarget::Dependency(binding) = binding {
            imports.push(Self::editing_dependency_import(binding));
        }
        self.observe_type_use(self.current_file, name.span, target, false, imports);
    }

    pub(crate) fn observe_type_name(&mut self, span: ast::Span, target: TypeTarget) {
        self.observe_type_use(self.current_file, span, target, false, Vec::new());
    }

    pub(crate) fn observe_type_declaration(
        &mut self,
        file: usize,
        span: ast::Span,
        target: TypeTarget,
    ) {
        if !self.observes_types() {
            return;
        }
        self.editing_type_definitions.insert(
            target,
            hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span,
            },
        );
        self.observe_type_use(file, span, target, true, Vec::new());
    }

    pub(crate) fn fresh_source_type_param(
        &mut self,
        declaration: &ast::TypeParamDecl,
        substitution_slot: usize,
    ) -> hir::TypeParamId {
        let id = self.fresh_type_param(substitution_slot);
        self.observe_type_declaration(
            self.current_file,
            declaration.name.span,
            TypeTarget::Parameter(id),
        );
        id
    }

    fn observe_type_use(
        &mut self,
        file: usize,
        span: ast::Span,
        target: TypeTarget,
        is_declaration: bool,
        imports: Vec<hir::EditingImportSource>,
    ) {
        if !self.observes_types() || span.start == span.end {
            return;
        }
        let observation = self.editing_types.as_mut().unwrap_or_else(|| {
            &mut self
                .editing_body
                .as_mut()
                .expect("body type observation")
                .types
        });
        let reference = observation.uses.entry((file, span)).or_insert(TypeUse {
            target,
            is_declaration,
            imports: Vec::new(),
        });
        if reference.target == target {
            for import in imports {
                if !reference.imports.contains(&import) {
                    reference.imports.push(import);
                }
            }
        }
    }
}
