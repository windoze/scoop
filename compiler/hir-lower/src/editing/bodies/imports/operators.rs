use super::*;
use crate::imports::ImportedTargetBinding;

impl Lowerer {
    pub(super) fn editing_operator_import_sources(
        &self,
        callable: hir::SemanticCallable,
        layer: ImportLookupLayer,
    ) -> Vec<hir::EditingImportSource> {
        let matches = |binding: &ImportedTargetBinding| matches!(self.editing_import_member(binding), Some(hir::EditingMemberTarget::Callable(target)) if target == callable);
        let file = match layer {
            ImportLookupLayer::BlockExact(depth) | ImportLookupLayer::BlockStar(depth) => {
                &*self.scopes.imports()[depth]
            }
            _ => &self.imports.files[&self.current_file],
        };
        match layer {
            ImportLookupLayer::Exact | ImportLookupLayer::BlockExact(_) => file
                .exact
                .iter()
                .filter(|import| import.targets.iter().any(matches))
                .map(|import| {
                    hir::EditingImportSource::Declaration(hir::SemanticSourceLocation {
                        source: import.origin.source.clone(),
                        span: import.origin.span,
                    })
                })
                .collect(),
            ImportLookupLayer::Star | ImportLookupLayer::BlockStar(_) => file
                .stars
                .iter()
                .flat_map(|import| {
                    import
                        .snapshot
                        .values()
                        .flat_map(|targets| targets.iter())
                        .filter(|binding| matches(binding))
                        .filter_map(|binding| binding.direct_binding())
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
                .values()
                .flatten()
                .filter(|binding| {
                    matches(&ImportedTargetBinding::DirectDependency((*binding).clone()))
                })
                .map(Self::editing_dependency_import)
                .collect(),
            ImportLookupLayer::QualifiedPackage | ImportLookupLayer::CorePrelude => Vec::new(),
        }
    }
}
