use super::*;

impl Lowerer {
    pub(super) fn register_function_parameter_interface(&mut self, function: hir::FunctionId) {
        let signature = self.signatures[&function].clone();
        let sources = signature
            .params
            .iter()
            .map(|parameter| ParameterSource {
                name: parameter.name.clone(),
                ty: parameter.ty,
                calling: parameter.calling.clone(),
            })
            .collect::<Vec<_>>();
        let owner = self.function_owner.get(&function).copied();
        let receiver = owner
            .map(|owner| (self.owner_ty(owner), Some(owner)))
            .or_else(|| {
                self.extension_receivers
                    .get(&function)
                    .copied()
                    .map(|ty| (ty, None))
            });
        let context = DefaultContext {
            definition_root: self
                .definition_root
                .unwrap_or(hir::LexicalDefinitionRoot::Function(function)),
            source_context: hir::SourceContextSubject::Function(function),
            type_parameters: signature.type_params,
            receiver,
            is_suspend: signature.is_suspend,
            safety: signature.attributes.safety,
            callable_name: self.functions[function].name.clone(),
        };
        self.register_export_parameter_interface(
            hir::ExportParameterOwner::Function(function),
            &sources,
            &context,
        );
    }
}
