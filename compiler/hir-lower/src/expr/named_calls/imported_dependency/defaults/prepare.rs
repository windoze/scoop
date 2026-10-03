//! Load each default definition once; expansion uses the ordinary HIR body.

use scoop_hir as hir;
use scoop_identity::SignatureTypeKey;
use std::sync::Arc;

use super::plan::{ImportedDefaultPlanError, PreparedImportedDefault};
use crate::Lowerer;
use crate::imported_core::ImportedTypeBindings;

impl Lowerer {
    pub(super) fn prepare_imported_default_with_bindings(
        &mut self,
        owner: &dyn hir::ImportedCallableSource,
        template: hir::ExportDefaultTemplateV1,
        bindings: &ImportedTypeBindings,
    ) -> Result<PreparedImportedDefault, ImportedDefaultPlanError> {
        let arguments = template
            .type_parameters()
            .arguments()
            .iter()
            .map(|argument| self.imported_default_type_with_bindings(argument, bindings))
            .collect::<Result<_, _>>()?;
        self.prepare_imported_default_with_arguments(owner, &template, arguments)
    }

    pub(crate) fn prepare_imported_default_with_arguments(
        &mut self,
        owner: &dyn hir::ImportedCallableSource,
        template: &hir::ExportDefaultTemplateV1,
        arguments: Vec<hir::TypeId>,
    ) -> Result<PreparedImportedDefault, ImportedDefaultPlanError> {
        let key = (
            template.definition_root(),
            template.definition_path().clone(),
        );
        let expression = if let Some(expression) = self.loaded_default_expressions.get(&key) {
            Arc::clone(expression)
        } else {
            let expression = Arc::new(
                self.load_default_expression(owner, template)
                    .map_err(|error| ImportedDefaultPlanError::Body(error.to_string()))?,
            );
            self.loaded_default_expressions
                .insert(key, Arc::clone(&expression));
            expression
        };
        Ok(PreparedImportedDefault {
            expression,
            arguments,
        })
    }

    pub(super) fn imported_default_type_with_bindings(
        &mut self,
        signature: &SignatureTypeKey,
        bindings: &ImportedTypeBindings,
    ) -> Result<hir::TypeId, ImportedDefaultPlanError> {
        self.imported_generic_type(signature, bindings)
            .map_err(ImportedDefaultPlanError::Body)
    }
}
