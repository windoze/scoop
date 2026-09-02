use super::*;

impl Lowerer {
    /// Register a generic definition once and return its typed id.
    pub(crate) fn register_generic(
        &mut self,
        function: FunctionId,
        parameters: Vec<hir::TypeParamDecl>,
    ) -> GenericFunctionId {
        if let hir::FunctionGenericity::Generic {
            definition,
            parameters: existing,
        } = &self.functions[function].genericity
        {
            assert_eq!(existing, &parameters);
            return *definition;
        }
        assert!(!parameters.is_empty());
        let generic = self.generic_functions.alloc(GenericFunction {
            function,
            no_gc_type_params: Vec::new(),
        });
        self.functions[function].genericity = hir::FunctionGenericity::Generic {
            definition: generic,
            parameters,
        };
        generic
    }

    /// Record a resolved generic application, deduplicated by
    /// (generic definition, type arguments), and return the entity id
    /// carried by the export HIR call. Requests from inside generic bodies
    /// may still mention `Type::Param`; the local-concrete HIR pass resolves
    /// them when the requesting instance is materialized.
    pub(crate) fn record_instantiation(
        &mut self,
        function: FunctionId,
        type_args: Vec<TypeId>,
    ) -> hir::ResolvedGenericFunctionId {
        let Some(generic) = self.functions[function].generic_definition() else {
            unreachable!("only a generic function can be instantiated")
        };
        if let Some((id, _)) = self
            .instantiations
            .iter()
            .find(|(_, request)| request.generic == generic && request.type_args == type_args)
        {
            return id;
        }
        self.instantiations
            .alloc(hir::ResolvedGenericFunction { generic, type_args })
    }

    pub(crate) fn register_method_parameters(
        &mut self,
        function: FunctionId,
        owner_parameters: Vec<hir::TypeParamDecl>,
        method_parameters: Vec<hir::TypeParamDecl>,
    ) {
        if method_parameters.is_empty() {
            self.functions[function].genericity = if owner_parameters.is_empty() {
                hir::FunctionGenericity::Plain
            } else {
                hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters,
                    no_gc_type_params: Vec::new(),
                }
            };
            return;
        }
        let method_parameters = hir::NonEmptyVec::from_vec(method_parameters)
            .expect("generic method declarations have a non-empty method parameter group");
        let definition = self.generic_methods.alloc(hir::GenericMethod {
            function,
            no_gc_type_params: Vec::new(),
        });
        self.functions[function].genericity = hir::FunctionGenericity::GenericMethod {
            definition,
            owner_parameters,
            method_parameters,
        };
    }

    pub(crate) fn record_method_application(
        &mut self,
        function: FunctionId,
        owner: hir::MethodOwnerApplication,
    ) -> hir::MethodApplicationId {
        let key = (function, owner);
        if let Some(&application) = self.method_application_by_key.get(&key) {
            return application;
        }
        let application = self
            .method_applications
            .alloc(hir::MethodApplication { function, owner });
        self.method_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn record_generic_method_application(
        &mut self,
        function: FunctionId,
        owner: hir::GenericMethodOwner,
        method_arguments: Vec<TypeId>,
    ) -> hir::GenericMethodApplicationId {
        let method = self.functions[function]
            .generic_method_definition()
            .expect("only a generic method has a generic method application");
        let method_arguments = hir::NonEmptyVec::from_vec(method_arguments)
            .expect("generic method applications have method arguments");
        let key = (method, owner, method_arguments.clone());
        if let Some(&application) = self.generic_method_application_by_key.get(&key) {
            return application;
        }
        let application = self
            .generic_method_applications
            .alloc(hir::GenericMethodApplication {
                method,
                owner,
                method_arguments,
            });
        self.generic_method_application_by_key
            .insert(key, application);
        application
    }

    pub(crate) fn materialize_candidate_callable(
        &mut self,
        candidate: &CallableCandidate,
        type_arguments: &[TypeId],
    ) -> hir::Callable {
        match &candidate.owner {
            CallableCandidateOwner::Function { .. } => {
                match self.functions[candidate.function].genericity {
                    hir::FunctionGenericity::Plain => hir::Callable::Function(candidate.function),
                    hir::FunctionGenericity::Generic { .. } => hir::Callable::Generic(
                        self.record_instantiation(candidate.function, type_arguments.to_vec()),
                    ),
                    hir::FunctionGenericity::OwnerParameterizedMethod { .. }
                    | hir::FunctionGenericity::GenericMethod { .. } => {
                        unreachable!("a method candidate has an exact method owner")
                    }
                }
            }
            CallableCandidateOwner::Method(owner) => {
                match &self.functions[candidate.function].genericity {
                    hir::FunctionGenericity::Plain
                    | hir::FunctionGenericity::OwnerParameterizedMethod { .. } => {
                        hir::Callable::Method(
                            self.record_method_application(candidate.function, *owner),
                        )
                    }
                    hir::FunctionGenericity::GenericMethod {
                        method_parameters, ..
                    } => {
                        let method_arguments = method_parameters
                            .iter()
                            .map(|parameter| type_arguments[parameter.id.into_raw() as usize])
                            .collect();
                        hir::Callable::GenericMethod(self.record_generic_method_application(
                            candidate.function,
                            self.generic_method_owner(*owner),
                            method_arguments,
                        ))
                    }
                    hir::FunctionGenericity::Generic { .. } => {
                        unreachable!("generic functions do not have nominal method owners")
                    }
                }
            }
        }
    }

    pub(crate) fn callable_function_id(&self, callable: hir::Callable) -> FunctionId {
        match callable {
            hir::Callable::Function(function) => function,
            hir::Callable::Generic(instantiation) => {
                let generic = self.instantiations[instantiation].generic;
                self.generic_functions[generic].function
            }
            hir::Callable::Method(application) => self.method_applications[application].function,
            hir::Callable::GenericMethod(application) => {
                let method = self.generic_method_applications[application].method;
                self.generic_methods[method].function
            }
        }
    }

    /// The host type of a member-function owner: the class / interface
    /// / struct type, or the enum applied to its own type parameters
    /// (the form `this` has inside the enum's methods).
    pub(crate) fn owner_ty(&mut self, owner: Owner) -> TypeId {
        match owner {
            Owner::Class(id) => {
                self.class_applications[self.classes[id].self_application].canonical_type
            }
            Owner::Interface(id) => {
                self.interface_applications[self.interfaces[id].self_application].canonical_type
            }
            Owner::Struct(id) => {
                self.struct_applications[self.structs[id].self_application].canonical_type
            }
            Owner::Enum(id) => {
                self.enum_applications[self.enums[id].self_application].canonical_type
            }
        }
    }

    pub(crate) fn owner_type_args(&self, owner: Owner) -> Vec<TypeId> {
        match owner {
            Owner::Class(id) => self.class_applications[self.classes[id].self_application]
                .arguments
                .clone(),
            Owner::Interface(id) => self.interface_applications
                [self.interfaces[id].self_application]
                .arguments
                .clone(),
            Owner::Struct(id) => self.struct_applications[self.structs[id].self_application]
                .arguments
                .clone(),
            Owner::Enum(id) => self.enum_applications[self.enums[id].self_application]
                .arguments
                .clone(),
        }
    }

    pub(crate) fn method_owner_application(
        &mut self,
        owner: Owner,
        arguments: Vec<TypeId>,
    ) -> hir::MethodOwnerApplication {
        match owner {
            Owner::Class(id) => {
                hir::MethodOwnerApplication::Class(self.class_application_id(id, arguments))
            }
            Owner::Struct(id) => {
                hir::MethodOwnerApplication::Struct(self.struct_application_id(id, arguments))
            }
            Owner::Enum(id) => {
                hir::MethodOwnerApplication::Enum(self.enum_application_id(id, arguments))
            }
            Owner::Interface(id) => {
                hir::MethodOwnerApplication::Interface(self.interface_application_id(id, arguments))
            }
        }
    }

    pub(crate) fn method_owner_arguments(&self, owner: hir::MethodOwnerApplication) -> &[TypeId] {
        match owner {
            hir::MethodOwnerApplication::Class(id) => &self.class_applications[id].arguments,
            hir::MethodOwnerApplication::Struct(id) => &self.struct_applications[id].arguments,
            hir::MethodOwnerApplication::Enum(id) => &self.enum_applications[id].arguments,
            hir::MethodOwnerApplication::Interface(id) => {
                &self.interface_applications[id].arguments
            }
        }
    }

    pub(crate) fn callable_candidate_owner_arguments(
        &self,
        candidate: &CallableCandidate,
    ) -> Vec<TypeId> {
        match &candidate.owner {
            CallableCandidateOwner::Function { owner_arguments } => owner_arguments.clone(),
            CallableCandidateOwner::Method(owner) => self.method_owner_arguments(*owner).to_vec(),
        }
    }

    pub(crate) fn generic_method_owner(
        &self,
        owner: hir::MethodOwnerApplication,
    ) -> hir::GenericMethodOwner {
        match owner {
            hir::MethodOwnerApplication::Class(id) => hir::GenericMethodOwner::Class(id),
            hir::MethodOwnerApplication::Struct(id) => hir::GenericMethodOwner::Struct(id),
            hir::MethodOwnerApplication::Enum(id) => hir::GenericMethodOwner::Enum(id),
            hir::MethodOwnerApplication::Interface(_) => {
                unreachable!("generic methods have class, struct, or enum owners")
            }
        }
    }

    /// Type parameters contributed by a member's owning declaration. They
    /// form the prefix of the member function's combined parameter space.
    pub(crate) fn owner_type_params(&self, owner: Owner) -> Vec<hir::TypeParamDecl> {
        match owner {
            Owner::Class(id) => self.classes[id].type_params.clone(),
            Owner::Struct(id) => self.structs[id].type_params.clone(),
            Owner::Enum(id) => self.enums[id].type_params.clone(),
            Owner::Interface(id) => self.interfaces[id].type_params.clone(),
        }
    }
}
