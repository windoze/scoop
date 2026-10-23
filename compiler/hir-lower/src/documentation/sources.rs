use super::*;

impl Lowerer {
    pub(super) fn documentation_source_locations(
        &self,
        declarations: impl Iterator<Item = (Target, hir::DefinitionOrigin)>,
    ) -> std::collections::HashMap<hir::DocumentationSymbol, hir::SemanticSourceLocation> {
        let mut locations = declarations
            .map(|(target, origin)| {
                (
                    hir::DocumentationSymbol::Current(target),
                    hir::SemanticSourceLocation {
                        source: self.source_identity(origin.file as usize).clone(),
                        span: origin.span,
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        if let Some(dependencies) = &self.dependencies {
            locations.extend(dependencies.declaration_locations().into_iter().filter_map(
                |(target, origin)| {
                    crate::semantic_queries::imported_location(origin.origin())
                        .ok()
                        .map(|origin| (hir::DocumentationSymbol::Imported(target), origin))
                },
            ));
        }
        locations
    }
}
