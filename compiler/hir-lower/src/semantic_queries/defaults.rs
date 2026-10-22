use super::*;
use hir::{ImportedCallableSource, SemanticQueries};

mod provider;
mod references;

impl FrontendQueries<'_, '_> {
    pub(super) fn query_default_application(
        &mut self,
        context: hir::SemanticTypeContext<'_>,
        reference: hir::SemanticDefaultReference,
        arguments: &[hir::TypeId],
    ) -> Result<Option<hir::SemanticDefaultApplication>, Vec<Diagnostic>> {
        let Some(definition) = self.body_queries.default(reference).map_err(one)? else {
            return Ok(None);
        };
        let (provider, parameters) = self.default_provider(reference).map_err(one)?;
        let types = arguments
            .iter()
            .map(|&ty| self.type_signature(context, ty))
            .collect::<Result<Vec<_>, _>>()
            .map_err(one)?;
        let types =
            hir::CanonicalBinderUseListV1::try_new(types).map_err(|cause| one(error(cause)))?;
        let application = hir::SemanticDefaultApplication::try_new(definition, provider, types)
            .map_err(|cause| one(error(cause)))?;
        self.lowerer.in_type_context(context, |lowerer| {
            let before = lowerer.diagnostics.len();
            let valid = lowerer.check_type_argument_kinds(
                &parameters,
                arguments,
                Span::new(0, 0),
                "queried default provider",
            );
            let mut diagnostics = lowerer.diagnostics.split_off(before);
            let sources = lowerer.diagnostic_source_identities();
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            if valid && diagnostics.is_empty() {
                Ok(Some(application))
            } else {
                Err(diagnostics)
            }
        })
    }
}

fn one(diagnostic: Diagnostic) -> Vec<Diagnostic> {
    vec![diagnostic]
}

fn error(cause: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::at(
        Span::new(0, 0),
        format!("cannot project the queried default application: {cause}"),
    )
}
