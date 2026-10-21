use super::*;

impl FrontendQueries<'_> {
    pub(super) fn in_context<T>(
        &mut self,
        context: hir::SemanticContext,
        operation: impl FnOnce(&mut Lowerer) -> T,
    ) -> T {
        let (file, owner, parameters) = match context {
            hir::SemanticContext::Function(function) => (
                self.lowerer.function_files[&function],
                self.lowerer.function_owner.get(&function).copied(),
                self.lowerer.signatures[&function].type_params.clone(),
            ),
            hir::SemanticContext::Nominal(owner) => {
                let file = match owner {
                    hir::NominalOwner::Class(id) => self.lowerer.class_files[&id],
                    hir::NominalOwner::Struct(id) => self.lowerer.struct_files[&id],
                    hir::NominalOwner::Enum(id) => self.lowerer.enum_files[&id],
                    hir::NominalOwner::Interface(id) => self.lowerer.interface_files[&id],
                    hir::NominalOwner::Object(id) => self.lowerer.object_files[&id],
                };
                let owner = Owner::from_nominal_owner(owner);
                (file, Some(owner), self.lowerer.owner_type_params(owner))
            }
        };
        let previous_file = std::mem::replace(&mut self.lowerer.current_file, file);
        let previous_query = std::mem::replace(&mut self.lowerer.in_semantic_query, true);
        let previous_owner = std::mem::replace(&mut self.lowerer.current_owner, owner);
        let previous_parameters =
            std::mem::replace(&mut self.lowerer.type_params_in_scope, parameters);
        let previous_scopes = owner
            .and_then(|owner| self.lowerer.local_nominal_scopes(owner))
            .map(|scopes| std::mem::replace(&mut self.lowerer.scopes, scopes));
        let previous_functions = owner
            .and_then(|owner| self.lowerer.local_nominal_functions(owner))
            .map(|functions| std::mem::replace(&mut self.lowerer.local_function_scopes, functions));
        let value = operation(self.lowerer);
        if let Some(scopes) = previous_scopes {
            self.lowerer.scopes = scopes;
        }
        if let Some(functions) = previous_functions {
            self.lowerer.local_function_scopes = functions;
        }
        self.lowerer.current_file = previous_file;
        self.lowerer.in_semantic_query = previous_query;
        self.lowerer.current_owner = previous_owner;
        self.lowerer.type_params_in_scope = previous_parameters;
        value
    }

    pub(super) fn signature_binders(
        &self,
        context: hir::SemanticContext,
    ) -> Vec<hir::HirSignatureBinder> {
        let (parameters, owner_count) = match context {
            hir::SemanticContext::Function(function) => {
                let signature = &self.lowerer.signatures[&function];
                (
                    signature.type_params.clone(),
                    signature.owner_type_param_count,
                )
            }
            hir::SemanticContext::Nominal(owner) => {
                let parameters = self
                    .lowerer
                    .owner_type_params(Owner::from_nominal_owner(owner));
                let count = parameters.len();
                (parameters, count)
            }
        };
        let has_own = parameters.len() > owner_count;
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| hir::HirSignatureBinder {
                parameter: parameter.id,
                depth: u32::from(has_own && index < owner_count),
                index: u32::try_from(if index < owner_count {
                    index
                } else {
                    index - owner_count
                })
                .expect("declaration parameter counts fit the identity schema"),
            })
            .collect()
    }
}
