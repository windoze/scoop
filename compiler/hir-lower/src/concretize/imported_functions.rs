use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_callable_application(
        &mut self,
        application: &export::ImportedGenericCallableApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionId {
        let (owner, arguments) = match &application.arguments {
            export::ImportedCallableArguments::Function(arguments) => (
                None,
                arguments
                    .iter()
                    .map(|ty| self.lower_type(*ty, substitution))
                    .collect(),
            ),
            export::ImportedCallableArguments::Method {
                owner,
                method_arguments,
            } => {
                let ty = self.lower_type(*owner, substitution);
                let owner = match self.types[ty].kind {
                    concrete::TypeKind::Class(id) => concrete::MethodOwner::Class(id),
                    concrete::TypeKind::Struct(id) => concrete::MethodOwner::Struct(id),
                    concrete::TypeKind::Enum(id) => concrete::MethodOwner::Enum(id),
                    concrete::TypeKind::Interface(id) => concrete::MethodOwner::Interface(id),
                    concrete::TypeKind::Ptr(_) => concrete::MethodOwner::TypeOwned(ty),
                    _ => unreachable!("a method has a nominal owner"),
                };
                let mut arguments = self.concrete_method_owner_arguments(owner).to_vec();
                arguments.extend(
                    method_arguments
                        .iter()
                        .map(|ty| self.lower_type(*ty, substitution)),
                );
                (Some(owner), arguments)
            }
        };
        let source = FunctionSource::Imported(application.template);
        let key = self.function_key(source, owner, arguments);
        self.request_function_key(key, source)
    }

    pub(super) fn lower_imported_function(
        &mut self,
        source: export::ImportedGenericCallableTemplateId,
        arguments: &[concrete::TypeId],
    ) -> PendingFunction {
        let template = self.source.imported_generic_templates[source].clone();
        let (body, locals) = match &template.implementation {
            export::ImportedGenericCallableImplementation::Body(body) => {
                self.lower_body(body, arguments)
            }
            // The complete unit supplies the coordinator body in MIR lowering,
            // exactly as for a locally declared initialization ensure entry.
            export::ImportedGenericCallableImplementation::InitializationEnsure => (
                concrete::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                },
                Vec::new(),
            ),
        };
        let parameters: Vec<concrete::Param> = template
            .parameters
            .iter()
            .map(|parameter| concrete::Param {
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, arguments),
                local: locals[parameter.local.into_raw().into_u32() as usize],
            })
            .collect();
        let capture_parameters = match &template.declaration {
            export::ImportedCallableTemplateOrigin::Generic(_)
            | export::ImportedCallableTemplateOrigin::Closure { .. }
            | export::ImportedCallableTemplateOrigin::Initialization { .. }
            | export::ImportedCallableTemplateOrigin::ExtensionAccessor(_)
            | export::ImportedCallableTemplateOrigin::Nominal { .. } => Vec::new(),
            export::ImportedCallableTemplateOrigin::Local {
                capture_bindings, ..
            } => capture_bindings
                .iter()
                .zip(&parameters)
                .map(|(binding, parameter)| concrete::LocalCaptureParameter {
                    binding: concrete::BindingId::from_raw(binding.into_raw()),
                    local: parameter.local,
                })
                .collect(),
        };
        let receiver = match template.receiver {
            Some(ty)
                if matches!(
                    template.declaration,
                    export::ImportedCallableTemplateOrigin::Nominal { .. }
                ) =>
            {
                let owner = self.lower_type(ty, arguments);
                let export::ImportedCallableTemplateOrigin::Nominal {
                    modifier, dispatch, ..
                } = template.declaration
                else {
                    unreachable!("a nominal receiver has a nominal declaration");
                };
                let dispatch = match dispatch {
                    export::ImportedMethodDispatch::Direct => concrete::MethodDispatch::Direct,
                    export::ImportedMethodDispatch::Virtual(family) => {
                        concrete::MethodDispatch::Virtual(self.lower_virtual_method(family))
                    }
                    export::ImportedMethodDispatch::FinalOverride(family) => {
                        concrete::MethodDispatch::FinalOverride(self.lower_virtual_method(family))
                    }
                    export::ImportedMethodDispatch::Interface(slot) => {
                        let concrete::TypeKind::Interface(interface) = self.types[owner].kind
                        else {
                            unreachable!("interface method dispatch retains its exact owner");
                        };
                        concrete::MethodDispatch::Interface {
                            interface,
                            slot: self.interface_slot_by_source[&(interface, slot)],
                        }
                    }
                };
                concrete::FunctionReceiver::Method(concrete::Method {
                    owner,
                    modifier,
                    dispatch,
                })
            }
            Some(ty) => concrete::FunctionReceiver::Extension(self.lower_type(ty, arguments)),
            None => concrete::FunctionReceiver::None,
        };
        PendingFunction {
            name: template.name.clone(),
            is_suspend: template.effects.execution() == scoop_identity::Effect::Suspend,
            modifiers: template.effects.callable_modifiers(),
            params: parameters,
            capture_parameters,
            return_ty: self.lower_type(template.return_type, arguments),
            attributes: template.effects.function_attributes(),
            kind: concrete::FunctionKind::User(body),
            receiver,
            span: template.span,
        }
    }
}
