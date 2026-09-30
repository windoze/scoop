use super::*;

pub(super) struct ResolvedFunctionDefinition<'a> {
    pub name: &'a str,
    pub is_suspend: bool,
    pub modifiers: export::CallableModifiers,
    pub parameters: &'a [export::Param],
    pub capture_bindings: Vec<export::BindingId>,
    pub return_type: export::TypeId,
    pub attributes: export::FunctionAttributes,
    pub implementation: &'a export::FunctionKind,
    pub receiver: DefinitionReceiver,
    pub span: scoop_ast::Span,
}

pub(super) enum DefinitionReceiver {
    None,
    Extension(export::TypeId),
    Method {
        owner: export::TypeId,
        modifier: export::MethodModifier,
        dispatch: export::DeclaredMethodDispatch,
    },
}

impl<'input> Concretizer<'input> {
    pub(super) fn resolved_function_definition(
        &self,
        key: &FunctionKey,
    ) -> ResolvedFunctionDefinition<'input> {
        match self.function_source(key) {
            FunctionSource::Local(id) => {
                let source = &self.source.functions[id];
                let receiver = match source.method {
                    Some(method) => DefinitionReceiver::Method {
                        owner: method.owner,
                        modifier: method.modifier,
                        dispatch: self.declared_method_dispatch(method.dispatch),
                    },
                    None if self.source_function_has_extension_receiver(id) => {
                        DefinitionReceiver::Extension(
                            source
                                .params
                                .first()
                                .expect("an extension has its receiver")
                                .ty,
                        )
                    }
                    None => DefinitionReceiver::None,
                };
                ResolvedFunctionDefinition {
                    name: &source.name,
                    is_suspend: source.is_suspend,
                    modifiers: source.modifiers,
                    parameters: &source.params,
                    capture_bindings: self.local_capture_bindings(id),
                    return_type: source.return_ty,
                    attributes: source.attributes,
                    implementation: &source.kind,
                    receiver,
                    span: source.span,
                }
            }
            FunctionSource::Imported(id) => {
                let source = &self.source.imported_generic_templates[id];
                let receiver = match (source.receiver, &source.declaration) {
                    (
                        Some(owner),
                        export::ImportedCallableTemplateOrigin::Nominal {
                            modifier, dispatch, ..
                        },
                    ) => DefinitionReceiver::Method {
                        owner,
                        modifier: *modifier,
                        dispatch: *dispatch,
                    },
                    (Some(receiver), _) => DefinitionReceiver::Extension(receiver),
                    (None, _) => DefinitionReceiver::None,
                };
                let capture_bindings = match &source.declaration {
                    export::ImportedCallableTemplateOrigin::Local {
                        capture_bindings, ..
                    } => capture_bindings.clone(),
                    _ => Vec::new(),
                };
                ResolvedFunctionDefinition {
                    name: &source.name,
                    is_suspend: source.effects.execution() == scoop_identity::Effect::Suspend,
                    modifiers: source.effects.callable_modifiers(),
                    parameters: &source.parameters,
                    capture_bindings,
                    return_type: source.return_type,
                    attributes: source.effects.function_attributes(),
                    implementation: &source.implementation,
                    receiver,
                    span: source.span,
                }
            }
        }
    }

    fn declared_method_dispatch(
        &self,
        dispatch: export::MethodDispatch,
    ) -> export::DeclaredMethodDispatch {
        match dispatch {
            export::MethodDispatch::Direct => export::DeclaredMethodDispatch::Direct,
            export::MethodDispatch::Virtual(family) => {
                export::DeclaredMethodDispatch::Virtual(family)
            }
            export::MethodDispatch::FinalOverride(family) => {
                export::DeclaredMethodDispatch::FinalOverride(family)
            }
            export::MethodDispatch::Interface(member) => export::DeclaredMethodDispatch::Interface(
                self.source.dispatch_slot_identities[member].id(),
            ),
        }
    }

    fn source_function_has_extension_receiver(&self, source: export::FunctionId) -> bool {
        match &self.source.function_identities[source] {
            export::HirFunctionIdentity::Source(identity) => identity
                .declaration()
                .duplicate_signature()
                .receiver_is_present(),
            export::HirFunctionIdentity::PropertyAccessor(accessor) => {
                let property = match accessor {
                    export::HirPropertyAccessorFunction::Getter(getter) => {
                        self.source.property_accessor_identities.get_getter(*getter)
                    }
                    export::HirPropertyAccessorFunction::Setter(setter) => {
                        self.source.property_accessor_identities.get_setter(*setter)
                    }
                }
                .expect("a property accessor retains its original identity")
                .property();
                self.source.property_identities[property]
                    .extension_id()
                    .is_some()
            }
            export::HirFunctionIdentity::LexicalGenerated(_)
            | export::HirFunctionIdentity::Initialization { .. }
            | export::HirFunctionIdentity::DerivedEquality(_) => false,
        }
    }
}
