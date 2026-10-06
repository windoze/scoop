use super::*;

pub(in crate::concretize) enum ResolvedFunctionDefinition<'a> {
    Body(ResolvedBodyDefinition<'a>),
    Companion {
        template: export::ImportedCompanionTemplateId,
        role: scoop_identity::InitializationCallableRole,
        signature: &'a export::CallableSignature,
    },
}

impl ResolvedFunctionDefinition<'_> {
    pub(in crate::concretize) fn signature(&self) -> &export::CallableSignature {
        match self {
            Self::Body(body) => body.signature,
            Self::Companion { signature, .. } => signature,
        }
    }
}

pub(in crate::concretize) struct ResolvedBodyDefinition<'a> {
    pub signature: &'a export::CallableSignature,
    pub capture_bindings: Vec<export::BindingId>,
    pub implementation: &'a export::FunctionKind,
    pub receiver: DefinitionReceiver,
}

pub(in crate::concretize) enum DefinitionReceiver {
    None,
    Extension(export::TypeId),
    Method {
        owner: export::TypeId,
        modifier: export::MethodModifier,
        dispatch: export::DeclaredMethodDispatch,
    },
}

impl<'input> Concretizer<'input> {
    pub(in crate::concretize) fn resolved_function_definition(
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
                ResolvedFunctionDefinition::Body(ResolvedBodyDefinition {
                    signature: &source.signature,
                    capture_bindings: self.local_capture_bindings(id),
                    implementation: &source.kind,
                    receiver,
                })
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
                ResolvedFunctionDefinition::Body(ResolvedBodyDefinition {
                    signature: &source.signature.signature,
                    capture_bindings,
                    implementation: &source.implementation,
                    receiver,
                })
            }
            FunctionSource::Companion(template, role) => ResolvedFunctionDefinition::Companion {
                template,
                role,
                signature: &self.source.imported_companion_templates[template].signature,
            },
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
