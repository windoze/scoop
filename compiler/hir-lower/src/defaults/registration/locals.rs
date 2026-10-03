use super::*;

impl Lowerer {
    pub(crate) fn lower_local_parameter_interface(
        &mut self,
        function: hir::FunctionId,
        source_name: &str,
    ) {
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
        let context = DefaultContext {
            definition_root: self
                .definition_root
                .unwrap_or(hir::LexicalDefinitionRoot::Function(function)),
            source_context: hir::SourceContextSubject::Function(function),
            type_parameters: signature.type_params,
            receiver: None,
            is_suspend: signature.is_suspend,
            safety: signature.attributes.safety,
            callable_name: source_name.to_owned(),
        };
        let environment = super::preparation::LocalDefaultEnvironment {
            available: self.capture_environment(),
            functions: self.local_function_scopes.clone(),
        };
        self.register_local_default_parameters(function, sources, context, environment);
    }
}
