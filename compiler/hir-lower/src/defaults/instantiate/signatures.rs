use super::*;

impl Lowerer {
    pub(in crate::defaults) fn instantiate_default_local_signature(
        &mut self,
        source: hir::LocalFunctionId,
        outer_bindings: &[(hir::TypeParamId, hir::TypeId)],
    ) -> hir::FunctionTypeId {
        let function = &self.local_functions[source];
        let signature = self.function_types[function.function_type].canonical_type;
        let parameters = match function.definition {
            hir::LexicalFunctionDefinition::Source { function, .. } => self.functions[function]
                .type_params()
                .into_iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>(),
            hir::LexicalFunctionDefinition::Template(template) => self.imported_generic_templates
                [template]
                .type_parameters
                .ids(),
        };
        let own_parameters = parameters.into_iter().skip(function.owner_type_param_count);
        let mut bindings = outer_bindings.to_vec();
        bindings.extend(
            own_parameters
                .into_iter()
                .map(|parameter| (parameter, self.intern_type(Type::Param(parameter)))),
        );
        let signature = self.instantiate_method_ty(signature, &bindings);
        let Type::Function(function_type) = self.types[signature] else {
            unreachable!("local function signature substitution preserves its kind")
        };
        function_type
    }
}
