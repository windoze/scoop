use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_delegate_reference(
        &mut self,
        reference: &hir::DefaultGenericDelegateReferenceV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::GenericDelegateReference, ImportedDefaultMaterializationError> {
        if matches!(
            context.evaluation,
            ImportedTemplateEvaluation::DefaultUse(_)
        ) {
            return Err(ImportedDefaultMaterializationError::InvalidControlFlow(
                "default properties require accessor calls, not direct delegate storage",
            ));
        }
        let template = self
            .request_imported_generic_delegate(reference.property())
            .map_err(ImportedDefaultMaterializationError::Plan)?;
        let initializer = self.imported_generic_delegate_templates[template].initializer;
        if reference.arguments().len()
            != self.imported_generic_templates[initializer]
                .type_parameters
                .len()
        {
            return Err(ImportedDefaultMaterializationError::InvalidControlFlow(
                "delegate storage arguments differ from its property binders",
            ));
        }
        let arguments = reference
            .arguments()
            .iter()
            .map(|argument| {
                self.imported_generic_type(argument, context.bindings)
                    .map_err(ImportedDefaultMaterializationError::Plan)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(hir::GenericDelegateReference {
            template: hir::GenericDelegateTemplateSource::Imported(template),
            arguments: hir::NonEmptyVec::from_vec(arguments)
                .expect("a portable delegate reference has nonempty arguments"),
        })
    }
}
