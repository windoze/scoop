use super::*;

mod imported;

impl Lowerer {
    pub(crate) fn imported_callable_owner_parameters(
        &mut self,
        owner: hir::SourceNominalId,
    ) -> Result<Vec<hir::TypeParamDecl>, crate::imported_core::ImportedSignatureTypeError> {
        let declaration = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(owner))
            .ok_or(crate::imported_core::ImportedSignatureTypeError::Structural)?;
        if declaration.interface.type_parameters().binders().is_empty() {
            Ok(Vec::new())
        } else {
            self.nominal_declaration_parameters(owner)
        }
    }

    pub(crate) fn semantic_function_signature<R, P>(
        &self,
        function: hir::FunctionId,
        result: R,
        parameters: Vec<P>,
    ) -> hir::SemanticFunctionSignature<R, P> {
        let signature = &self.signatures[&function];
        hir::SemanticFunctionSignature {
            function,
            name: self.source_function_name(function).into(),
            receiver: self.functions[function]
                .method
                .map(|method| method.owner)
                .or_else(|| self.extension_receivers.get(&function).copied()),
            parameters,
            type_parameters: signature.type_params.clone(),
            owner_parameter_count: signature.owner_type_param_count,
            context_parameters: signature.context_parameters.clone(),
            result,
            is_suspend: signature.is_suspend,
            modifiers: signature.modifiers,
            attributes: signature.attributes,
            release_callability: self.functions[function].release_callability.clone(),
            origin: hir::SemanticSourceLocation {
                source: self
                    .source_identity(
                        self.function_definition_file(function)
                            .expect("source callable signatures retain their definition file"),
                    )
                    .clone(),
                span: self.functions[function].span,
            },
        }
    }
}
