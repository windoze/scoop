use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_imported_callable_application(
        &mut self,
        application: &export::ImportedGenericCallableApplication,
        substitution: &[concrete::TypeId],
    ) -> concrete::FunctionId {
        let source = application.template;
        let key = match &application.arguments {
            export::ImportedCallableArguments::Function(arguments) => FunctionKey::Imported {
                source,
                arguments: arguments
                    .iter()
                    .map(|ty| self.lower_type(*ty, substitution))
                    .collect(),
            },
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
                    _ => unreachable!("an imported method has a nominal owner"),
                };
                FunctionKey::ImportedMethod {
                    source,
                    owner,
                    method_arguments: method_arguments
                        .iter()
                        .map(|ty| self.lower_type(*ty, substitution))
                        .collect(),
                }
            }
        };
        self.request_function_key(key)
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
            export::ImportedCallableTemplateOrigin::Local { parent, descriptor } => descriptor
                .captures()
                .iter()
                .zip(&parameters)
                .map(|(capture, parameter)| concrete::LocalCaptureParameter {
                    binding: self.imported_capture_binding(*parent, capture.source()),
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

    fn imported_capture_binding(
        &self,
        parent: export::ImportedCallableTemplateParent,
        source: &export::DefaultCaptureSourceV1,
    ) -> concrete::BindingId {
        let template = match parent {
            export::ImportedCallableTemplateParent::Function(template) => template,
            export::ImportedCallableTemplateParent::Constructor(template) => {
                return self.imported_constructor_capture_binding(template, source);
            }
        };
        let template = &self.source.imported_generic_templates[template];
        let capture_index = match source {
            export::DefaultCaptureSourceV1::Local(selector) => {
                let (id, local) = template
                    .source_body()
                    .locals
                    .iter()
                    .find(|(_, local)| &local.selector == selector)
                    .expect("provider captures retain their actual outer value selector");
                let index = match &template.declaration {
                    export::ImportedCallableTemplateOrigin::Generic(_)
                    | export::ImportedCallableTemplateOrigin::Closure { .. }
                    | export::ImportedCallableTemplateOrigin::Initialization { .. }
                    | export::ImportedCallableTemplateOrigin::ExtensionAccessor(_)
                    | export::ImportedCallableTemplateOrigin::Nominal { .. } => None,
                    export::ImportedCallableTemplateOrigin::Local { descriptor, .. } => template
                        .parameters[..descriptor.capture_count() as usize]
                        .iter()
                        .position(|parameter| parameter.local == id),
                };
                match index {
                    Some(index) => index,
                    None => return concrete::BindingId::from_raw(local.binding.into_raw()),
                }
            }
            export::DefaultCaptureSourceV1::EnclosingCapture(index) => *index as usize,
        };
        match &template.declaration {
            export::ImportedCallableTemplateOrigin::Local { parent, descriptor } => self
                .imported_capture_binding(*parent, descriptor.captures()[capture_index].source()),
            export::ImportedCallableTemplateOrigin::Closure {
                capture_bindings, ..
            } => concrete::BindingId::from_raw(capture_bindings[capture_index].into_raw()),
            _ => panic!("an enclosing capture belongs to a lexical implementation"),
        }
    }
    fn imported_constructor_capture_binding(
        &self,
        template: export::ImportedConstructorTemplateId,
        source: &export::DefaultCaptureSourceV1,
    ) -> concrete::BindingId {
        let template = &self.source.imported_constructor_templates[template];
        let export::DefaultCaptureSourceV1::Local(selector) = source else {
            unreachable!("constructor captures name source inputs or locals")
        };
        let binding = match selector {
            scoop_identity::LocalValueSelector::This => {
                unreachable!("a constructor cannot capture its initializing receiver")
            }
            scoop_identity::LocalValueSelector::Parameter { declaration_index } => {
                template.parameters[*declaration_index as usize].binding
            }
            _ => {
                let (body, arguments) = match &template.kind {
                    export::ImportedConstructorKind::ClassTerminal { body, base } => (
                        body,
                        match base {
                            export::BaseInitialization::Root => None,
                            export::BaseInitialization::Super { arguments, .. } => Some(arguments),
                        },
                    ),
                    export::ImportedConstructorKind::ClassThis {
                        body, arguments, ..
                    }
                    | export::ImportedConstructorKind::StructSecondary {
                        body, arguments, ..
                    } => (body, Some(arguments)),
                    export::ImportedConstructorKind::StructPrimary => {
                        unreachable!("a primary struct constructor has no lexical body")
                    }
                };
                body.locals
                    .values()
                    .chain(arguments.into_iter().flat_map(|args| args.locals.values()))
                    .find(|local| &local.selector == selector)
                    .expect("constructor captures reference their original lexical locals")
                    .binding
            }
        };
        concrete::BindingId::from_raw(binding.into_raw())
    }
}
