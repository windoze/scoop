use super::*;

mod captures;
mod definition;

use definition::DefinitionReceiver;

pub(super) struct PendingFunction {
    pub(super) name: String,
    pub(super) is_suspend: bool,
    pub(super) modifiers: export::CallableModifiers,
    pub(super) params: Vec<concrete::Param>,
    pub(super) capture_parameters: Vec<concrete::LocalCaptureParameter>,
    pub(super) return_ty: concrete::TypeId,
    pub(super) attributes: export::FunctionAttributes,
    pub(super) kind: concrete::FunctionKind,
    pub(super) receiver: concrete::FunctionReceiver,
    pub(super) span: scoop_ast::Span,
}

impl PendingFunction {
    pub(super) fn finish(
        self,
        materialization: concrete::CallableMaterialization,
    ) -> concrete::Function {
        concrete::Function {
            name: self.name,
            materialization,
            is_suspend: self.is_suspend,
            modifiers: self.modifiers,
            params: self.params,
            capture_parameters: self.capture_parameters,
            return_ty: self.return_ty,
            attributes: self.attributes,
            kind: self.kind,
            receiver: self.receiver,
            span: self.span,
        }
    }
}

impl Concretizer<'_> {
    pub(super) fn is_emittable_source_function(&self, id: export::FunctionId) -> bool {
        let function = &self.source.functions[id];
        if !matches!(
            function.kind,
            export::FunctionKind::User(_)
                | export::FunctionKind::Abstract { .. }
                | export::FunctionKind::InitializationEnsure
                | export::FunctionKind::DerivedEquality
                | export::FunctionKind::Extern(_)
        ) {
            return false;
        }
        let Some(method) = function.method else {
            return true;
        };
        !matches!(
            self.source.types[method.owner],
            export::Type::Interface(..) | export::Type::Any
        )
    }

    pub(super) fn lower_function(&mut self, key: &FunctionKey) -> PendingFunction {
        if self.type_use_site.is_none()
            && let FunctionSource::Local(id) = self.function_source(key)
        {
            self.type_use_site = self.source_context_site(
                export::SourceContextSubject::Function(id),
                self.source.functions[id].span,
            );
        }
        let definition = self.resolved_function_definition(key);
        let signature = definition.signature;
        let arguments = self.function_key_arguments(key);
        let (kind, local_map) = match definition.implementation {
            export::FunctionKind::User(body) => {
                let (body, locals) = self.lower_body(body, &arguments);
                (concrete::FunctionKind::User(body), Some(locals))
            }
            export::FunctionKind::Abstract { locals } => {
                let (locals, local_map) = self.lower_locals(locals, &arguments);
                (concrete::FunctionKind::Abstract { locals }, Some(local_map))
            }
            export::FunctionKind::DerivedEquality => {
                let (body, locals) = self.derived_bodies.get(key).cloned().expect(
                    "a typed derived application supplies its concrete body before emission",
                );
                (concrete::FunctionKind::User(body), Some(locals))
            }
            export::FunctionKind::Intrinsic(intrinsic) => {
                (concrete::FunctionKind::Intrinsic(*intrinsic), None)
            }
            export::FunctionKind::Extern(external) => (
                concrete::FunctionKind::Extern(self.extern_map[external]),
                None,
            ),
            export::FunctionKind::InitializationEnsure => {
                (concrete::FunctionKind::InitializationEnsure, None)
            }
        };
        let params: Vec<concrete::Param> = signature
            .params
            .iter()
            .map(|parameter| concrete::Param {
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, &arguments),
                local: match &local_map {
                    Some(locals) => locals[parameter.local.into_raw().into_u32() as usize],
                    None => remap_idx(parameter.local),
                },
            })
            .collect();
        let return_ty = self.lower_type(signature.return_ty, &arguments);
        let receiver = self.lower_definition_receiver(definition.receiver, &arguments);
        let capture_parameters = definition
            .capture_bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| concrete::LocalCaptureParameter {
                binding: concrete::BindingId::from_raw(binding.into_raw()),
                local: params[index].local,
            })
            .collect();
        PendingFunction {
            name: signature.name.to_owned(),
            is_suspend: signature.is_suspend,
            modifiers: signature.modifiers,
            params,
            capture_parameters,
            return_ty,
            attributes: signature.attributes,
            kind,
            receiver,
            span: signature.span,
        }
    }

    fn lower_definition_receiver(
        &mut self,
        receiver: DefinitionReceiver,
        arguments: &[concrete::TypeId],
    ) -> concrete::FunctionReceiver {
        match receiver {
            DefinitionReceiver::None => concrete::FunctionReceiver::None,
            DefinitionReceiver::Extension(receiver) => {
                concrete::FunctionReceiver::Extension(self.lower_type(receiver, arguments))
            }
            DefinitionReceiver::Method {
                owner,
                modifier,
                dispatch,
            } => {
                let owner = self.lower_type(owner, arguments);
                let dispatch = match dispatch {
                    export::DeclaredMethodDispatch::Direct => concrete::MethodDispatch::Direct,
                    export::DeclaredMethodDispatch::Virtual(family) => {
                        concrete::MethodDispatch::Virtual(self.lower_virtual_method(family))
                    }
                    export::DeclaredMethodDispatch::FinalOverride(family) => {
                        concrete::MethodDispatch::FinalOverride(self.lower_virtual_method(family))
                    }
                    export::DeclaredMethodDispatch::Interface(slot) => {
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
        }
    }
}
