use super::*;

impl Lowerer {
    pub(super) fn materialize_context_lookup(
        &mut self,
        declaration: hir::DefaultCallableDeclarationV1,
        parameter: hir::ContextParameterIndex,
        diagnostic: &hir::ContextDiagnostic,
        result_type: &scoop_identity::SignatureTypeKey,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let ImportedTemplateSource::Callable(source) = &context.owner else {
            return Err(ImportedDefaultMaterializationError::Plan(
                "context lookup must belong to a callable implementation".into(),
            ));
        };
        let body = source.body();
        if body.owner() != declaration
            || body
                .context_parameters()
                .get(parameter.0 as usize)
                .is_none_or(|requirement| requirement.value_type() != result_type)
        {
            return Err(ImportedDefaultMaterializationError::Plan(
                "context lookup does not match its declaration and requirement index".into(),
            ));
        }
        self.prepare_missing_context_exception_type()
            .map_err(|error| {
                ImportedDefaultMaterializationError::Plan(
                    error.diagnostic("missing context exception type"),
                )
            })?;
        Ok(hir::ExprKind::ContextLookup(hir::ContextRequirementRef {
            declaration: hir::ContextRequirementOwner::Imported(declaration),
            parameter,
            diagnostic: diagnostic.clone(),
        }))
    }
}
