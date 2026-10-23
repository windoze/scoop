//! The editor binds the same comments without projecting incomplete signatures.

use super::*;
use crate::editing::EditingCancelled;

impl Lowerer {
    pub(crate) fn finish_editing_documentation(
        &mut self,
        world: &hir::ImportedSemanticWorld<'_>,
        check: &impl Fn() -> Result<(), EditingCancelled>,
    ) -> Result<Option<hir::SemanticDocumentation>, EditingCancelled> {
        let Some(context) = self.documentation.take() else {
            return Ok(None);
        };
        check()?;
        let context = context.into_owned();
        let index = DocumentationIndex::new(&context.declarations);
        let source_locations = self.documentation_source_locations(
            context
                .declarations
                .iter()
                .map(|entry| (entry.declaration.target, entry.declaration.origin)),
        );
        let mut resolver = self.clone();
        resolver.scopes = crate::Scopes::new();
        resolver.diagnostics.clear();
        let mut warnings = context.warnings;
        let mut seen = HashSet::new();
        let mut declarations = Vec::with_capacity(context.declarations.len());
        for captured in context.declarations {
            check()?;
            resolver.current_file = captured.scope.file;
            resolver.current_owner = captured.scope.owner;
            resolver.type_params_in_scope = captured.scope.parameters;
            let target = captured.declaration.target;
            let comment = captured.comment.map(|comment| {
                resolver.bind_documentation(
                    target,
                    comment,
                    &index,
                    Some(world),
                    &mut warnings,
                    &mut seen,
                )
            });
            declarations.push(hir::SemanticDocumentationDeclaration { target, comment });
        }
        check()?;
        let sources = self.diagnostic_source_identities();
        for warning in &mut warnings {
            warning.resolve_sources(&sources);
        }
        self.warnings.extend(warnings.iter().cloned());
        Ok(Some(hir::SemanticDocumentation {
            declarations,
            source_locations,
            warnings,
            exported_targets: Default::default(),
        }))
    }
}
