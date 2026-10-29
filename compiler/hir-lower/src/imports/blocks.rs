//! Imports become ordinary candidates at their lexical statement position.

use super::*;
use crate::Lowerer;

impl Lowerer {
    pub(crate) fn lower_block_import(&mut self, import: &ast::ImportSyntax) {
        let (exposure, span) = match import {
            ast::ImportSyntax::Exact { exposure, span, .. }
            | ast::ImportSyntax::Star { exposure, span, .. } => (*exposure, *span),
        };
        if let ast::ImportExposureSyntax::PublicReexport {
            public_keyword_span,
        } = exposure
        {
            self.error(
                public_keyword_span,
                "public import is only allowed in a file header".into(),
            );
            return;
        }
        let previous = self
            .scopes
            .imports()
            .last()
            .expect("block import scope")
            .clone();
        let mut surface = (*self.imports).clone();
        let resolved =
            surface.resolve_imports(self, std::slice::from_ref(import), &previous.exact, None);
        if self.editing_types.is_some() || self.editing_body.is_some() {
            surface
                .block_declarations
                .insert((self.current_file, span.start, span.end), resolved.clone());
        }
        self.imports = surface.into();
        self.scopes.add_imports(resolved);
    }
}

impl CurrentUnitImports {
    pub(super) fn block_layers(
        &self,
        lowerer: &Lowerer,
        name: Option<&str>,
    ) -> Vec<ImportCandidateLayer> {
        let mut layers = Vec::new();
        for (depth, scope) in lowerer.scopes.imports().iter().enumerate().rev() {
            if scope.exact.is_empty() && scope.stars.is_empty() && scope.unavailable.is_empty() {
                continue;
            }
            for (kind, targets, unavailable) in [
                (
                    ImportLookupLayer::BlockExact(depth),
                    scope
                        .exact
                        .iter()
                        .filter(|import| name.is_none_or(|name| import.local_name == name))
                        .flat_map(|import| import.targets.iter())
                        .collect::<Vec<_>>(),
                    scope
                        .unavailable
                        .iter()
                        .filter(|import| {
                            import
                                .local_name
                                .as_deref()
                                .is_some_and(|local| name.is_none_or(|name| local == name))
                        })
                        .map(|import| import.binding)
                        .collect::<Vec<_>>(),
                ),
                (
                    ImportLookupLayer::BlockStar(depth),
                    scope
                        .stars
                        .iter()
                        .flat_map(|import| {
                            import
                                .snapshot
                                .iter()
                                .filter(|(key, _)| name.is_none_or(|name| key.as_str() == name))
                                .map(|(_, targets)| targets)
                        })
                        .flat_map(ast::NonEmptyVec::iter)
                        .collect(),
                    scope
                        .unavailable
                        .iter()
                        .filter(|import| import.local_name.is_none())
                        .map(|import| import.binding)
                        .collect(),
                ),
            ] {
                layers.push(ImportCandidateLayer {
                    kind,
                    bindings: self.canonicalize(
                        targets
                            .iter()
                            .filter_map(|target| target.current_binding())
                            .chain(unavailable),
                    ),
                    dependency_bindings: self.canonicalize_dependencies(
                        targets
                            .iter()
                            .filter_map(|target| target.direct_binding())
                            .cloned(),
                    ),
                    suppressed_callables: Vec::new(),
                    suppressed_values: Vec::new(),
                });
            }
        }
        layers
    }
}
