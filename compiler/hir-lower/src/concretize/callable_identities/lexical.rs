use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn lexical_context(
        &mut self,
        function: export::FunctionId,
        site: &LexicalSite,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        if let Some(parent) = self.immediate_parent_function(function, site) {
            return self.function_context(parent, inherited_arguments);
        }
        match site.root {
            export::LexicalDefinitionRoot::Function(parent) => {
                self.function_context(parent, inherited_arguments)
            }
            export::LexicalDefinitionRoot::ClassConstructor(constructor) => {
                self.class_constructor_context(constructor, inherited_arguments)
            }
            export::LexicalDefinitionRoot::StructConstructor(constructor) => {
                self.struct_constructor_context(constructor, inherited_arguments)
            }
            export::LexicalDefinitionRoot::VariantConstructor(variant) => {
                self.variant_constructor_context(variant, inherited_arguments)
            }
        }
    }

    fn immediate_parent_function(
        &self,
        function: export::FunctionId,
        site: &LexicalSite,
    ) -> Option<export::FunctionId> {
        let segments = site.path.segments();
        let mut candidate = None;
        for (possible_parent, possible_site) in &self.lexical_sites {
            let possible_segments = possible_site.path.segments();
            if *possible_parent == function
                || possible_site.root != site.root
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

    fn function_context(
        &mut self,
        source: export::FunctionId,
        inherited_arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        let expected = self.concretizer.source.functions[source].type_param_count();
        assert!(expected <= inherited_arguments.len());
        let arguments = &inherited_arguments[..expected];
        let candidates = self
            .concretizer
            .function_keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| {
                (key.source() == source
                    && self.concretizer.function_key_arguments(key) == arguments)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let [index] = candidates.as_slice() else {
            panic!(
                "a lexical parent has exactly one concrete materialization, found {}",
                candidates.len()
            )
        };
        self.resolve_function(*index).context()
    }

    fn class_constructor_context(
        &mut self,
        constructor: export::ClassConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        let declaration = &self.concretizer.source.class_constructors[constructor];
        let owner = self.class_owner_exact(declaration.owner, arguments);
        match &self.concretizer.source.constructor_identities[constructor] {
            export::HirClassConstructorIdentity::Source(record) => {
                self.constructor_application_context(record.id(), owner, arguments)
            }
            export::HirClassConstructorIdentity::ZeroArgumentAdapter { source, .. } => {
                let source = &self.concretizer.source.constructor_identities[*source];
                let record = source
                    .source_record()
                    .expect("a zero-argument adapter references a source constructor");
                self.constructor_application_context(record.id(), owner, arguments)
            }
        }
    }

    fn struct_constructor_context(
        &mut self,
        constructor: export::StructConstructorId,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        let declaration = &self.concretizer.source.struct_constructors[constructor];
        let owner = self.struct_owner_exact(declaration.owner, arguments);
        let persistent = self.concretizer.source.constructor_identities[constructor].id();
        self.constructor_application_context(persistent, owner, arguments)
    }

    fn constructor_application_context(
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

    fn variant_constructor_context(
        &mut self,
        variant: export::EnumVariantRef,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterializationContext {
        if arguments.is_empty() {
            return CallableMaterializationContext::NoSubstitution;
        }
        let exact_owner = self.enum_owner_exact(variant.enumeration(), arguments);
        let origin = self.concretizer.source.enum_member_identities[variant].id();
        let application = self.record_application(CallableApplicationKey::for_variant_constructor(
            origin,
            CallableInstantiationOwner::ExactNominalOwner(exact_owner),
        ));
        CallableMaterializationContext::Application(application)
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
            concrete::MethodOwner::Structural(ty) => ty,
        };
        self.exact_types[ty].id()
    }

    fn class_owner_exact(
        &self,
        owner: export::ClassId,
        arguments: &[concrete::TypeId],
    ) -> scoop_identity::PersistentExactTypeId {
        assert_eq!(
            self.concretizer.source.classes[owner].type_params.len(),
            arguments.len()
        );
        let local = self.concretizer.class_by_key[&(owner, arguments.to_vec())];
        self.exact_types[self.concretizer.class_type[&local]].id()
    }

    fn struct_owner_exact(
        &self,
        owner: export::StructId,
        arguments: &[concrete::TypeId],
    ) -> scoop_identity::PersistentExactTypeId {
        assert_eq!(
            self.concretizer.source.structs[owner].type_params.len(),
            arguments.len()
        );
        let local = self.concretizer.struct_by_key[&(owner, arguments.to_vec())];
        self.exact_types[self.concretizer.struct_type[&local]].id()
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
