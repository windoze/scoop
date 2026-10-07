use super::*;

mod constructors;
mod definition;

use definition::{ResolvedClassDefinition, ResolvedClassRepresentation};

impl Concretizer<'_> {
    pub(super) fn ensure_class_definition(
        &mut self,
        origin: export::SourceNominalId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::ClassId {
        let key = (origin, arguments.clone());
        if let Some(&id) = self.class_by_key.get(&key) {
            return id;
        }
        let previous_site = self.type_use_site;
        self.type_use_site = previous_site.or_else(|| self.source_nominal_site(origin));
        let source = self.source.nominal_identities.class_id(origin);
        let definition = match source {
            Some(source) => self.source_class_definition(source, application),
            None => ResolvedClassDefinition::from_dependency(
                &self.source.loaded_class_definitions[&origin],
                application,
            ),
        };
        let id = self.allocate_class_definition(&definition, arguments.clone());
        let method_owner = if let Some(source) = source {
            self.class_source.insert(id, source);
            if let Some(object) = self.object_by_backing_class.get(&source).copied() {
                self.register_object(object, id, self.class_type[&id]);
                concrete::MethodOwner::Object(self.object_type_map[&id])
            } else {
                concrete::MethodOwner::Class(id)
            }
        } else {
            if let Some((template, _)) = self
                .source
                .imported_companion_templates
                .iter()
                .find(|(_, template)| template.declaration.owner() == origin)
            {
                self.register_imported_companion(template, id);
            }
            concrete::MethodOwner::Class(id)
        };
        self.complete_class_definition(id, definition, &arguments, method_owner);
        self.materialize_release_hook(id, origin, &arguments);
        if let Some(imported) = self.source.loaded_class_definitions.get(&origin) {
            self.check_loaded_contexts(&imported.context_contracts, &arguments);
        }
        self.type_use_site = previous_site;
        id
    }

    fn allocate_class_definition(
        &mut self,
        definition: &ResolvedClassDefinition<'_>,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::ClassId {
        let key = (definition.origin.declaration_id(), arguments.clone());
        let id = concrete::ClassId::from_raw(
            u32::try_from(self.classes.len())
                .expect("concrete class ids fit in u32")
                .into(),
        );
        let representation = match &definition.representation {
            ResolvedClassRepresentation::Declared { .. } => {
                concrete::ClassRepresentation::Declared {
                    fields: Vec::new(),
                    base_class: None,
                }
            }
            ResolvedClassRepresentation::Intrinsic {
                declaration,
                application,
            } => concrete::ClassRepresentation::Intrinsic {
                declaration: *declaration,
                application: application.clone(),
            },
        };
        let kind = match &representation {
            concrete::ClassRepresentation::Declared { .. }
            | concrete::ClassRepresentation::Intrinsic {
                application:
                    concrete::IntrinsicTypeRepresentation::Array { .. }
                    | concrete::IntrinsicTypeRepresentation::MutableArray { .. }
                    | concrete::IntrinsicTypeRepresentation::Nothing,
                ..
            } => concrete::TypeKind::Class(id),
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::String,
                ..
            } => concrete::TypeKind::String,
            concrete::ClassRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Any,
                ..
            } => concrete::TypeKind::Any,
            _ => unreachable!("the registry fixes intrinsic class representations"),
        };
        let ty = self.intern_type(kind, false);
        let allocated = self.classes.alloc(concrete::ClassDef {
            release_policy: concrete::ReleasePolicy::None,
            origin: definition.origin.clone(),
            canonical_type: ty,
            modifier: definition.modifier,
            name: definition.name.clone(),
            owner: definition.owner.clone(),
            type_arguments: arguments,
            representation,
            direct_interfaces: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: definition.span,
        });
        assert_eq!(allocated, id);
        self.class_by_key.insert(key, id);
        self.class_type.insert(id, ty);
        id
    }

    fn complete_class_definition(
        &mut self,
        id: concrete::ClassId,
        definition: ResolvedClassDefinition<'_>,
        substitution: &[concrete::TypeId],
        method_owner: concrete::MethodOwner,
    ) {
        if let ResolvedClassRepresentation::Declared { fields, base_class } =
            definition.representation
        {
            let fields = fields
                .into_iter()
                .map(|field| concrete::Field {
                    identity: field.identity,
                    name: field.name.to_owned(),
                    ty: self.lower_type(field.ty, substitution),
                })
                .collect();
            let base_class = base_class.map(|base| {
                let ty = self.lower_type(base, substitution);
                let concrete::TypeKind::Class(base) = self.types[ty].kind else {
                    unreachable!("a resolved class base retains its class type")
                };
                base
            });
            self.classes[id].representation =
                concrete::ClassRepresentation::Declared { fields, base_class };
        }
        let mut methods = self
            .request_concrete_methods(definition.methods, method_owner)
            .into_iter()
            .map(concrete::ClassMethod::Local)
            .collect::<Vec<_>>();
        let direct_interfaces: Vec<_> = definition
            .interfaces
            .iter()
            .map(|interface| self.lower_type(*interface, substitution))
            .collect();
        methods.extend(
            definition
                .virtual_methods
                .iter()
                .map(|method| match method.callable {
                    export::ImportedDispatchCallable::External(callable) => {
                        concrete::ClassMethod::Imported {
                            family: self.lower_virtual_method(method.family),
                            callable: self.imported_dependency_callable_map[&callable],
                        }
                    }
                    export::ImportedDispatchCallable::Template(application) => {
                        concrete::ClassMethod::Local(self.lower_imported_callable_application(
                            &self.source.imported_generic_applications[application],
                            substitution,
                        ))
                    }
                }),
        );
        let interface_implementations = self
            .lower_interface_implementations(definition.interface_implementations, substitution);
        self.classes[id].interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        self.classes[id].interface_implementations = interface_implementations;
        self.classes[id].direct_interfaces = direct_interfaces;
        self.classes[id].methods = methods;
        for &constructor in definition.constructors {
            if self.automatic_class_constructor(constructor) {
                self.request_class_constructor(constructor, id);
            }
        }
    }

    pub(super) fn request_concrete_methods(
        &mut self,
        source_methods: &[export::FunctionId],
        owner: concrete::MethodOwner,
    ) -> Vec<concrete::FunctionId> {
        let substitution = self.concrete_method_owner_arguments(owner).to_vec();
        for &method in source_methods {
            self.check_source_method_context(method, &substitution);
        }
        source_methods
            .iter()
            .copied()
            .filter_map(|method| {
                let function = &self.source.functions[method];
                if !self.automatic_method(method) {
                    return None;
                }
                match function.genericity {
                    export::FunctionGenericity::Plain
                    | export::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                        Some(self.request_method(method, owner, Vec::new()))
                    }
                    export::FunctionGenericity::GenericMethod { .. } => None,
                    export::FunctionGenericity::Generic { .. } => {
                        unreachable!("nominal method lists do not contain generic functions")
                    }
                }
            })
            .collect()
    }

    pub(super) fn lower_interface_implementations(
        &mut self,
        source_implementations: &[export::InterfaceImplementation],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::InterfaceImplementation> {
        source_implementations
            .iter()
            .cloned()
            .map(|implementation| {
                let interface = self.lower_interface_type(implementation.interface, substitution);
                let methods = implementation
                    .methods
                    .into_iter()
                    .map(|method| {
                        let source_slot = self.interface_reference_slot(method.member);
                        let slot = self.interface_slot_by_source[&(interface, source_slot)];
                        let target = match method.target {
                            export::InterfaceImplementationTarget::ImportedTemplate(
                                application,
                            ) => concrete::InterfaceImplementationTarget::Method(
                                self.lower_imported_callable_application(
                                    &self.source.imported_generic_applications[application],
                                    substitution,
                                ),
                            ),
                            export::InterfaceImplementationTarget::ImportedAbstractTemplate(
                                application,
                            ) => concrete::InterfaceImplementationTarget::Abstract {
                                declaration: self.lower_imported_callable_application(
                                    &self.source.imported_generic_applications[application],
                                    substitution,
                                ),
                            },
                            export::InterfaceImplementationTarget::Method(application) => {
                                let concrete::Callable::Function(function) =
                                    self.lower_method_application(application, substitution);
                                concrete::InterfaceImplementationTarget::Method(function)
                            }
                            export::InterfaceImplementationTarget::Imported(callable) => {
                                concrete::InterfaceImplementationTarget::Imported(
                                    self.imported_dependency_callable_map[&callable],
                                )
                            }
                            export::InterfaceImplementationTarget::ImportedAbstract(callable) => {
                                concrete::InterfaceImplementationTarget::ImportedAbstract {
                                    declaration: self.imported_dependency_callable_map[&callable],
                                }
                            }
                            export::InterfaceImplementationTarget::Abstract(application) => {
                                let concrete::Callable::Function(declaration) =
                                    self.lower_method_application(application, substitution);
                                concrete::InterfaceImplementationTarget::Abstract { declaration }
                            }
                        };
                        concrete::InterfaceMethodImplementation { slot, target }
                    })
                    .collect();
                concrete::InterfaceImplementation { interface, methods }
            })
            .collect()
    }

    pub(super) fn lower_interface_type(
        &mut self,
        source: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> concrete::InterfaceId {
        let ty = self.lower_type(source, substitution);
        let concrete::TypeKind::Interface(interface) = self.types[ty].kind else {
            unreachable!("interface type lowers to an interface")
        };
        interface
    }
}
