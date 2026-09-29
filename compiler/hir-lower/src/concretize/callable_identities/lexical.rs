use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn imported_materialization(
        &mut self,
        declaration: export::ImportedCallableTemplateOrigin,
        method_owner: Option<concrete::MethodOwner>,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        match declaration {
            export::ImportedCallableTemplateOrigin::Initialization { template, owner } => {
                let unit = self.concretizer.initialization_map[&InitializationKey {
                    source: initialization::InitializationSource::ImportedDelegate(template),
                    arguments: arguments.to_vec(),
                }];
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(owner),
                    CallableMaterializationContext::InitializationApplication(
                        self.concretizer.initialization_units[unit].identity.id(),
                    ),
                )
            }
            export::ImportedCallableTemplateOrigin::Generic(id) => self.source_materialization(
                SourceTemplate::GenericFunction(id),
                CallableInstantiationOwner::NoOwner,
                arguments,
            ),
            export::ImportedCallableTemplateOrigin::ExtensionAccessor(id) => self
                .source_materialization(
                    SourceTemplate::Accessor {
                        id,
                        extension: true,
                    },
                    CallableInstantiationOwner::NoOwner,
                    arguments,
                ),
            export::ImportedCallableTemplateOrigin::Nominal {
                declaration,
                owner_parameter_count,
                ..
            } => {
                let owner = method_owner.expect("a member application retains its concrete owner");
                let owner = if owner_parameter_count == 0 {
                    CallableInstantiationOwner::NoOwner
                } else {
                    CallableInstantiationOwner::ExactNominalOwner(self.exact_method_owner(owner))
                };
                let template = match declaration {
                    export::DefaultCallableDeclarationV1::Function(id) => {
                        SourceTemplate::Function(id)
                    }
                    export::DefaultCallableDeclarationV1::GenericFunction(id) => {
                        SourceTemplate::GenericFunction(id)
                    }
                    export::DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                        SourceTemplate::Accessor {
                            id,
                            extension: false,
                        }
                    }
                    export::DefaultCallableDeclarationV1::Generated(_) => {
                        unreachable!("nominal source bodies have source declarations")
                    }
                };
                self.source_materialization(template, owner, &arguments[owner_parameter_count..])
            }
            export::ImportedCallableTemplateOrigin::Local { parent, descriptor } => {
                let inherited = descriptor.owner_type_parameter_count() as usize;
                let parent = self.imported_parent_materialization(parent, &arguments[..inherited]);
                let owner = match parent.context() {
                    CallableMaterializationContext::NoSubstitution => {
                        CallableInstantiationOwner::NoOwner
                    }
                    CallableMaterializationContext::Application(id) => {
                        CallableInstantiationOwner::EnclosingCallableApplication(id)
                    }
                    CallableMaterializationContext::InitializationApplication(id) => {
                        CallableInstantiationOwner::EnclosingInitializationApplication(id)
                    }
                };
                let template = match descriptor.declaration() {
                    scoop_identity::CallableTemplateOrigin::Function(id) => {
                        SourceTemplate::Function(id)
                    }
                    scoop_identity::CallableTemplateOrigin::GenericFunction(id) => {
                        SourceTemplate::GenericFunction(id)
                    }
                    _ => unreachable!("local descriptors contain source function declarations"),
                };
                self.source_materialization(template, owner, &arguments[inherited..])
            }
            export::ImportedCallableTemplateOrigin::Closure { parent, body, .. } => {
                let parent = self.imported_parent_materialization(parent, arguments);
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(body),
                    parent.context(),
                )
            }
        }
    }

    pub(super) fn imported_parent_materialization(
        &mut self,
        parent: scoop_identity::CallableTemplateOwner,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        if let CallableTemplateOwner::Constructor(declaration) = parent {
            let template = self
                .concretizer
                .source
                .imported_constructor_templates
                .iter()
                .find(|(_, template)| template.declaration == declaration)
                .expect("a lexical constructor retains its original definition")
                .0;
            return self.imported_constructor_materialization(template, arguments);
        }
        self.function_definition_materialization(parent, arguments)
    }

    fn function_definition_materialization(
        &mut self,
        definition: CallableTemplateOwner,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let index = self
            .concretizer
            .function_keys
            .iter()
            .position(|key| key.template_owner() == Some(definition) && key.arguments == arguments)
            .expect("a lexical application retains its original parent and complete arguments");
        self.resolve_function(index)
    }

    pub(super) fn lexical_context(
        &mut self,
        function: export::FunctionId,
        site: &LexicalSite,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        self.enclosing_materialization(Some(function), site.root, &site.path, inherited_arguments)
            .context()
    }

    pub(super) fn callback_context(
        &mut self,
        callback: export::ForeignCallbackRegistrationId,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        let registration = &self.concretizer.source.foreign_callback_registrations[callback];
        self.enclosing_materialization(
            None,
            registration.definition_root,
            &registration.definition_path,
            inherited_arguments,
        )
        .context()
    }

    pub(super) fn enclosing_materialization(
        &mut self,
        excluded_function: Option<export::FunctionId>,
        root: export::LexicalDefinitionRoot,
        path: &scoop_identity::StructuralDefinitionPath,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        if let Some(parent) = self.immediate_parent_function(excluded_function, root, path) {
            return self.function_materialization(parent, inherited_arguments);
        }
        match root {
            export::LexicalDefinitionRoot::Function(parent) => {
                self.function_materialization(parent, inherited_arguments)
            }
            export::LexicalDefinitionRoot::ClassConstructor(constructor) => {
                self.class_constructor_materialization(constructor, inherited_arguments)
            }
            export::LexicalDefinitionRoot::StructConstructor(constructor) => {
                self.struct_constructor_materialization(constructor, inherited_arguments)
            }
            export::LexicalDefinitionRoot::VariantConstructor(variant) => {
                self.variant_constructor_materialization(variant, inherited_arguments)
            }
        }
    }

    fn immediate_parent_function(
        &self,
        excluded_function: Option<export::FunctionId>,
        root: export::LexicalDefinitionRoot,
        path: &scoop_identity::StructuralDefinitionPath,
    ) -> Option<export::FunctionId> {
        let segments = path.segments();
        let mut candidate = None;
        for (possible_parent, possible_site) in &self.lexical_sites {
            let possible_segments = possible_site.path.segments();
            if Some(*possible_parent) == excluded_function
                || possible_site.root != root
                || possible_segments.len() >= segments.len()
                || !segments.starts_with(possible_segments)
            {
                continue;
            }
            match candidate {
                Some((length, parent)) if length == possible_segments.len() => {
                    assert_eq!(parent, *possible_parent);
                }
                Some((length, _)) if length > possible_segments.len() => {}
                _ => candidate = Some((possible_segments.len(), *possible_parent)),
            }
        }
        candidate.map(|(_, parent)| parent)
    }

    fn function_materialization(
        &mut self,
        source: export::FunctionId,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let expected = self.concretizer.source.functions[source].type_param_count();
        assert!(expected <= inherited_arguments.len());
        let arguments = &inherited_arguments[..expected];
        let definition = self
            .concretizer
            .function_key(FunctionSource::Local(source), None, Vec::new())
            .template_owner()
            .expect("a lexical root has an original body definition");
        self.function_definition_materialization(definition, arguments)
    }

    pub(super) fn constructor_application_context(
        &mut self,
        constructor: scoop_identity::PersistentConstructorId,
        owner: scoop_identity::PersistentExactTypeId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        if arguments.is_empty() {
            CallableMaterializationContext::NoSubstitution
        } else {
            let application = self.record_application(CallableApplicationKey::for_constructor(
                constructor,
                CallableInstantiationOwner::ExactNominalOwner(owner),
            ));
            CallableMaterializationContext::Application(application)
        }
    }

    fn variant_constructor_materialization(
        &mut self,
        variant: export::EnumVariantRef,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let origin = self.concretizer.source.enum_member_identities[variant].id();
        let context = if arguments.is_empty() {
            CallableMaterializationContext::NoSubstitution
        } else {
            let exact_owner = self.enum_owner_exact(variant.enumeration(), arguments);
            let application =
                self.record_application(CallableApplicationKey::for_variant_constructor(
                    origin,
                    CallableInstantiationOwner::ExactNominalOwner(exact_owner),
                ));
            CallableMaterializationContext::Application(application)
        };
        CallableMaterialization::new(CallableTemplateOwner::VariantConstructor(origin), context)
    }

    pub(super) fn lexical_parent(
        &self,
        materialization: CallableMaterialization,
    ) -> scoop_identity::LexicalCallableParent {
        match materialization.template() {
            CallableTemplateOwner::Function(id) => {
                scoop_identity::LexicalCallableParent::function(id)
            }
            CallableTemplateOwner::GenericFunction(id) => {
                scoop_identity::LexicalCallableParent::generic_function(id)
            }
            CallableTemplateOwner::Constructor(id) => {
                scoop_identity::LexicalCallableParent::constructor(id)
            }
            CallableTemplateOwner::Accessor(id) => {
                scoop_identity::LexicalCallableParent::accessor(id)
            }
            CallableTemplateOwner::Generated(id) => self.generated_lexical_parent(id),
            CallableTemplateOwner::VariantConstructor(id) => {
                scoop_identity::LexicalCallableParent::variant_constructor(id)
            }
        }
    }

    fn generated_lexical_parent(
        &self,
        id: scoop_identity::PersistentGeneratedCallableId,
    ) -> scoop_identity::LexicalCallableParent {
        for (function, _) in self.concretizer.source.functions.iter() {
            let identity = &self.concretizer.source.function_identities[function];
            match identity {
                export::HirFunctionIdentity::LexicalGenerated(record)
                | export::HirFunctionIdentity::Initialization { record, .. }
                    if record.id() == id =>
                {
                    return scoop_identity::LexicalCallableParent::from_generated_key(record.key())
                        .expect("an enclosing generated source callable is a lexical parent");
                }
                export::HirFunctionIdentity::DerivedEquality(applications) => {
                    if let Some(record) = applications
                        .iter()
                        .map(export::HirDerivedEqualityFunctionIdentity::record)
                        .find(|record| record.id() == id)
                    {
                        return scoop_identity::LexicalCallableParent::from_generated_key(
                            record.key(),
                        )
                        .expect("an enclosing generated source callable is a lexical parent");
                    }
                }
                export::HirFunctionIdentity::Source(_)
                | export::HirFunctionIdentity::PropertyAccessor(_)
                | export::HirFunctionIdentity::LexicalGenerated(_)
                | export::HirFunctionIdentity::Initialization { .. } => {}
            }
        }
        for (constructor, _) in self.concretizer.source.class_constructors.iter() {
            if let Some(record) = self.concretizer.source.constructor_identities[constructor]
                .generated_record()
                .filter(|record| record.id() == id)
            {
                return scoop_identity::LexicalCallableParent::from_generated_key(record.key())
                    .expect("an enclosing generated constructor is a lexical parent");
            }
        }
        panic!("missing generated lexical parent for {id:?}")
    }

    pub(super) fn exact_method_owner(
        &self,
        owner: concrete::MethodOwner,
    ) -> scoop_identity::PersistentExactTypeId {
        let ty = match owner {
            concrete::MethodOwner::Class(owner) => self.concretizer.class_type[&owner],
            concrete::MethodOwner::Struct(owner) => self.concretizer.struct_type[&owner],
            concrete::MethodOwner::Enum(owner) => self.concretizer.enum_type[&owner],
            concrete::MethodOwner::Interface(owner) => self.concretizer.interface_type[&owner],
            concrete::MethodOwner::Object(owner) => {
                self.concretizer.object_types[owner].canonical_type
            }
            concrete::MethodOwner::TypeOwned(ty) => ty,
        };
        self.exact_types[ty].id()
    }

    fn enum_owner_exact(
        &self,
        owner: export::EnumId,
        arguments: &[concrete::TypeId],
    ) -> scoop_identity::PersistentExactTypeId {
        assert_eq!(
            self.concretizer.source.enums[owner].type_params.len(),
            arguments.len()
        );
        let local = self.concretizer.enum_by_key[&(owner, arguments.to_vec())];
        self.exact_types[self.concretizer.enum_type[&local]].id()
    }
}
