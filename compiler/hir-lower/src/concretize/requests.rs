//! Concrete requests are keyed by original definitions and complete arguments.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct FunctionKey {
    definition: FunctionDefinition,
    pub(super) owner: Option<concrete::MethodOwner>,
    pub(super) arguments: Vec<concrete::TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FunctionDefinition {
    Body(export::DefaultCallableDeclarationV1),
    DerivedEquality,
}

impl FunctionKey {
    pub(super) fn template_owner(&self) -> Option<scoop_identity::CallableTemplateOwner> {
        match self.definition {
            FunctionDefinition::Body(declaration) => Some(declaration.template_owner()),
            FunctionDefinition::DerivedEquality => None,
        }
    }
}

/// A record location is separate from a request's semantic identity.
#[derive(Clone, Copy)]
pub(super) enum FunctionSource {
    Local(export::FunctionId),
    Imported(export::ImportedGenericCallableTemplateId),
}

impl Concretizer<'_> {
    pub(super) fn request_lexical_function(
        &mut self,
        definition: export::LexicalFunctionDefinition,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::FunctionId {
        let source = match definition {
            export::LexicalFunctionDefinition::Source { function, .. } => {
                FunctionSource::Local(function)
            }
            export::LexicalFunctionDefinition::Template(template) => {
                FunctionSource::Imported(template)
            }
        };
        let key = self.function_key(source, None, arguments);
        self.request_function_key(key, source)
    }

    pub(super) fn function_key(
        &self,
        source: FunctionSource,
        owner: Option<concrete::MethodOwner>,
        arguments: Vec<concrete::TypeId>,
    ) -> FunctionKey {
        let definition = match source {
            FunctionSource::Local(source) => self.source_function_definition(source),
            FunctionSource::Imported(source) => FunctionDefinition::Body(
                self.source.imported_generic_templates[source]
                    .declaration
                    .body_owner(),
            ),
        };
        FunctionKey {
            definition,
            owner,
            arguments,
        }
    }

    fn source_function_definition(&self, source: export::FunctionId) -> FunctionDefinition {
        use export::DefaultCallableDeclarationV1 as Declaration;
        let definition = match &self.source.function_identities[source] {
            export::HirFunctionIdentity::Source(identity) => match identity {
                export::HirSourceFunctionIdentity::Plain(record) => {
                    Declaration::Function(record.id())
                }
                export::HirSourceFunctionIdentity::Generic(record) => {
                    Declaration::GenericFunction(record.id())
                }
            },
            export::HirFunctionIdentity::PropertyAccessor(accessor) => {
                let identity = match accessor {
                    export::HirPropertyAccessorFunction::Getter(getter) => {
                        self.source.property_accessor_identities.get_getter(*getter)
                    }
                    export::HirPropertyAccessorFunction::Setter(setter) => {
                        self.source.property_accessor_identities.get_setter(*setter)
                    }
                }
                .expect("a source accessor retains its original identity");
                Declaration::PropertyAccessor(identity.id())
            }
            export::HirFunctionIdentity::LexicalGenerated(record)
            | export::HirFunctionIdentity::Initialization { record, .. } => {
                Declaration::Generated(record.id())
            }
            export::HirFunctionIdentity::DerivedEquality(_) => {
                return FunctionDefinition::DerivedEquality;
            }
        };
        FunctionDefinition::Body(definition)
    }

    pub(super) fn function_source(&self, key: &FunctionKey) -> FunctionSource {
        let id = self.function_by_key[key];
        self.function_sources[id.into_raw().into_u32() as usize]
    }

    pub(super) fn request_function(
        &mut self,
        source: export::FunctionId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_none(),
            "method instances require an exact concrete owner"
        );
        let source = FunctionSource::Local(source);
        let key = self.function_key(source, None, arguments);
        self.request_function_key(key, source)
    }

    pub(super) fn request_method(
        &mut self,
        source: export::FunctionId,
        owner: concrete::MethodOwner,
        method_arguments: Vec<concrete::TypeId>,
    ) -> concrete::FunctionId {
        assert!(
            self.source.functions[source].method.is_some(),
            "method requests name a method declaration"
        );
        let mut arguments = self.concrete_method_owner_arguments(owner).to_vec();
        arguments.extend(method_arguments);
        let source = FunctionSource::Local(source);
        let key = self.function_key(source, Some(owner), arguments);
        self.request_function_key(key, source)
    }

    pub(super) fn request_function_key(
        &mut self,
        key: FunctionKey,
        source: FunctionSource,
    ) -> concrete::FunctionId {
        if let Some(&id) = self.function_by_key.get(&key) {
            return id;
        }
        let (parameter_count, emittable) = match source {
            FunctionSource::Local(source) => (
                self.source.functions[source].type_param_count(),
                self.is_emittable_source_function(source),
            ),
            FunctionSource::Imported(source) => (
                self.source.imported_generic_templates[source]
                    .type_parameters
                    .len(),
                !matches!(
                    self.source.imported_generic_templates[source].implementation,
                    export::FunctionKind::Intrinsic(_) | export::FunctionKind::Extern(_)
                ),
            ),
        };
        assert_eq!(parameter_count, key.arguments.len());
        let id = concrete::FunctionId::from_raw((self.function_slots.len() as u32).into());
        self.function_slots.push(None);
        self.function_keys.push(key.clone());
        self.function_sources.push(source);
        self.function_by_key.insert(key.clone(), id);
        self.pending_functions
            .push_back((key, id, self.type_use_site));
        if emittable {
            self.emitted_functions.push(id);
        }
        id
    }

    pub(super) fn function_key_arguments(&self, key: &FunctionKey) -> Vec<concrete::TypeId> {
        key.arguments.clone()
    }

    pub(super) fn concrete_method_owner_arguments(
        &self,
        owner: concrete::MethodOwner,
    ) -> &[concrete::TypeId] {
        match owner {
            concrete::MethodOwner::Class(id) => &self.classes[id].type_arguments,
            concrete::MethodOwner::Struct(id) => &self.structs[id].type_arguments,
            concrete::MethodOwner::Enum(id) => &self.enums[id].type_arguments,
            concrete::MethodOwner::Interface(id) => &self.interfaces[id].type_arguments,
            concrete::MethodOwner::Object(_) => &[],
            concrete::MethodOwner::TypeOwned(ty) => match &self.types[ty].kind {
                concrete::TypeKind::Ptr(pointee) => std::slice::from_ref(pointee),
                _ => &[],
            },
        }
    }

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
}
