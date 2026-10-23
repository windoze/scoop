use super::*;

impl Lowerer {
    pub(super) fn query_source_methods(
        &mut self,
        receiver: hir::TypeId,
        owner: Owner,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<Vec<hir::SemanticMethodDescription>, Diagnostic> {
        let methods = match owner {
            Owner::Struct(id) => self.structs[id].methods.clone(),
            Owner::Class(id) => self.classes[id].methods.clone(),
            Owner::Enum(id) => self.enums[id].methods.clone(),
            Owner::Object(id) => self.classes[self.objects[id].backing_class].methods.clone(),
            Owner::Interface(id) => {
                let interface = &self.interfaces[id];
                interface
                    .methods
                    .iter()
                    .filter_map(|member| {
                        let member = &self.interface_method_entities[*member];
                        (member.role == hir::InterfaceMemberRole::Function)
                            .then_some(member.function)
                    })
                    .chain(interface.private_methods.iter().copied())
                    .collect()
            }
        };
        methods
            .into_iter()
            .filter(|function| self.source_function_declarations.contains_key(function))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|function| self.query_source_method(function, receiver, substitutions))
            .collect()
    }

    fn query_source_method(
        &mut self,
        function: hir::FunctionId,
        receiver: hir::TypeId,
        substitutions: &[(hir::TypeParamId, hir::TypeId)],
    ) -> Result<hir::SemanticMethodDescription, Diagnostic> {
        let identity = match self.query_function_identity(function)? {
            hir::HirSourceFunctionIdentity::Plain(record) => {
                CallableTemplateOrigin::Function(record.id())
            }
            hir::HirSourceFunctionIdentity::Generic(record) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        let signature = self.signatures[&function].clone();
        let complete =
            self.complete_parameter_substitution(signature.type_params.iter(), substitutions);
        let substitutions = &complete;
        let declaration = self.functions[function].clone();
        let method = declaration.method.expect("nominal members have a receiver");
        let modality = match method.dispatch {
            hir::MethodDispatch::Interface(member) => {
                match self.interface_method_entities[member].implementation {
                    hir::InterfaceMemberImplementation::Body => {
                        hir::CallableModalityV1::InterfaceDefault
                    }
                    hir::InterfaceMemberImplementation::AbstractSlot => {
                        hir::CallableModalityV1::Abstract
                    }
                }
            }
            _ => match method.modifier {
                hir::MethodModifier::Final => hir::CallableModalityV1::Final,
                hir::MethodModifier::Open => hir::CallableModalityV1::Open,
                hir::MethodModifier::Abstract => hir::CallableModalityV1::Abstract,
            },
        };
        let (owner_parameters, parameters) = signature
            .type_params
            .split_at(signature.owner_type_param_count);
        let file = self
            .function_definition_file(function)
            .expect("source method retains its definition file");
        Ok(hir::SemanticMethodDescription {
            identity,
            callable: hir::SemanticCallable::Current(function),
            name: self.source_function_name(function).into(),
            receiver: hir::SemanticTypeUse {
                declaration: method.owner,
                applied: receiver,
            },
            owner_parameters: owner_parameters.to_vec(),
            type_parameters: self.query_method_type_parameters(parameters, substitutions),
            context_parameters: self
                .query_context_parameters(&signature.context_parameters, substitutions),
            parameters: self.query_current_parameters(
                hir::ExportParameterOwner::Function(function),
                identity,
                substitutions,
            ),
            result: self.query_type_use(signature.return_ty, substitutions),
            is_suspend: signature.is_suspend,
            modifiers: signature.modifiers,
            attributes: signature.attributes,
            release_callability: declaration.release_callability.clone(),
            modality,
            access: hir::SemanticMemberAccess {
                visibility: declaration.access.declared.into(),
                domain: declaration.access.lookup.0.clone(),
                accessible: self.function_is_accessible(function, Some(receiver)),
            },
            annotations: hir::SemanticAnnotationTarget::Current(
                hir::SourceAnnotationTarget::Callable(hir::SourceAnnotationCallable::Function(
                    function,
                )),
            ),
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(file).clone(),
                span: declaration.span,
            },
        })
    }
}
