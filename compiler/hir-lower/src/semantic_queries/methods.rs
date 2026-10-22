//! Read declaration members with the frontend's original parameter identities.

use super::*;
use scoop_identity::CallableTemplateOrigin;

mod imported;
mod source;

impl Lowerer {
    pub(super) fn query_nominal_methods(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticMethodDescription>>, Diagnostic> {
        let Some(nominal) = self.describe_semantic_nominal(ty)? else {
            return Ok(None);
        };
        let declaration = nominal.declaration;
        let parameters = nominal.parameters;
        let arguments = nominal.arguments;
        let substitutions = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        let mut methods = if let Some(owner) = self.nominal_owners.get(&declaration).copied() {
            self.query_source_methods(ty, owner, &substitutions)?
        } else {
            let owner = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(declaration))
                .expect("a resolved dependency nominal retains its declaration")
                .clone();
            self.query_imported_methods(ty, &owner, &parameters, &substitutions)?
        };
        methods.sort_by_key(|method| method.origin.span.start);
        Ok(Some(methods))
    }

    pub(super) fn query_function_identity(
        &mut self,
        function: hir::FunctionId,
    ) -> Result<hir::HirSourceFunctionIdentity, Diagnostic> {
        if let Some(identity) = self.query_source_function_identities.get(&function) {
            return Ok(identity.clone());
        }
        let identity = crate::persistent_functions::direct_source_identity(
            self,
            function,
            self.query_core_types(),
        )
        .map_err(|error| {
            let mut diagnostic = Diagnostic::at(error.span(), error.to_string());
            diagnostic.file = error.file();
            diagnostic
        })?;
        self.query_source_function_identities
            .insert(function, identity.clone());
        Ok(identity)
    }

    fn query_method_type_parameters(
        &mut self,
        parameters: &[hir::TypeParamDecl],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Vec<hir::SemanticTypeParameterUse> {
        parameters
            .iter()
            .map(|declaration| {
                let mut applied = declaration.clone();
                self.substitute_type_parameter(&mut applied, substitutions);
                hir::SemanticTypeParameterUse {
                    declaration: declaration.clone(),
                    applied,
                }
            })
            .collect()
    }

    fn query_context_parameters(
        &mut self,
        parameters: &[hir::ContextParameter],
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Vec<hir::SemanticContextParameterDescription> {
        parameters
            .iter()
            .map(|parameter| hir::SemanticContextParameterDescription {
                label: parameter.label.clone(),
                value_type: self.query_type_use(parameter.ty, substitutions),
            })
            .collect()
    }
}
