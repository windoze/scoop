use std::collections::HashSet;

use super::*;

struct MethodOwnerInfo {
    arguments: Vec<TypeId>,
    parameters: Vec<TypeParamDecl>,
    canonical_type: TypeId,
}

impl Validator<'_> {
    pub(super) fn method_callee_info(&self, callee: MethodCallee) -> Check<CallableInfo> {
        match callee {
            MethodCallee::Callable(callable) => self.callable_info(callable),
            MethodCallee::Bound(bound) => {
                let bound = checked_arena(&self.module.bound_callable_refs, bound)
                    .ok_or_else(|| invalid("protocol call has an invalid bound callee"))?;
                if !self.parameters.contains(&bound.receiver_parameter) {
                    return fail("bound protocol call references a parameter outside its body");
                }
                let signature = self.checked_function_type(bound.instantiated_signature)?;
                let mut info = match bound.source {
                    BoundCallableSource::Class {
                        bound: class_bound,
                        callable,
                    } => {
                        let parameter = self.parameter(bound.receiver_parameter)?;
                        if parameter
                            .class_bound()
                            .is_none_or(|candidate| candidate.application != class_bound)
                        {
                            return fail("bound protocol call does not match its class bound");
                        }
                        self.checked_class_application(class_bound)?;
                        let info = self.callable_info(callable)?;
                        let owner = self.callable_method_owner(callable)?;
                        self.validate_class_bound_owner(class_bound, owner)?;
                        info
                    }
                    BoundCallableSource::Interface {
                        bound: interface_bound,
                        member,
                    } => {
                        let parameter = self.parameter(bound.receiver_parameter)?;
                        self.validate_parameter_interface_bound(parameter, interface_bound)?;
                        let member_value = checked_arena(&self.module.interface_methods, member)
                            .ok_or_else(|| invalid("bound protocol call has an invalid member"))?;
                        let owner = checked_arena(&self.module.interfaces, member_value.owner)
                            .ok_or_else(|| invalid("bound protocol member has an invalid owner"))?;
                        if !owner.methods.contains(&member) {
                            return fail("bound protocol member is not declared by its owner");
                        }

                        let mut closure = Vec::new();
                        self.append_interface_closure(interface_bound, &mut closure)?;
                        let mut matched = None;
                        for application in closure {
                            if self.checked_interface_application(application)?.template
                                != member_value.owner
                            {
                                continue;
                            }
                            let candidate = self.ordinary_method_info(
                                member_value.function,
                                MethodOwnerApplication::Interface(application),
                            )?;
                            if self.bound_signature_matches(signature, &candidate)
                                && matched.replace(candidate).is_some()
                            {
                                return fail(
                                    "bound protocol signature matches multiple owner applications",
                                );
                            }
                        }
                        matched.ok_or_else(|| {
                            invalid("bound protocol signature does not match its interface member")
                        })?
                    }
                };
                if !self.bound_signature_matches(signature, &info) {
                    return fail("bound protocol signature differs from its selected member");
                }
                info.receiver = Some(ReceiverType::Parameter(bound.receiver_parameter));
                Ok(info)
            }
            MethodCallee::DerivedEquality(_) => {
                fail("iteration protocols cannot use a derived equality target")
            }
        }
    }

    pub(super) fn callable_info(&self, callable: Callable) -> Check<CallableInfo> {
        match callable {
            Callable::Function(function) => {
                let function_value = checked_arena(&self.module.functions, function)
                    .ok_or_else(|| invalid("callable has an invalid function"))?;
                if !matches!(function_value.genericity, FunctionGenericity::Plain) {
                    return fail("parameterized function lacks a resolved callable identity");
                }
                self.specialized_callable_info(function, &[])
            }
            Callable::Generic(resolved) => {
                let resolved = checked_arena(&self.module.instantiations, resolved)
                    .ok_or_else(|| invalid("callable has an invalid generic application"))?;
                let generic = checked_arena(&self.module.generic_functions, resolved.generic)
                    .ok_or_else(|| invalid("generic application has an invalid definition"))?;
                let function = checked_arena(&self.module.functions, generic.function)
                    .ok_or_else(|| invalid("generic definition has an invalid function"))?;
                let FunctionGenericity::Generic {
                    definition,
                    parameters,
                } = &function.genericity
                else {
                    return fail("generic application points to a non-generic function");
                };
                if function.method.is_some()
                    || *definition != resolved.generic
                    || parameters.len() != resolved.type_args.len()
                    || resolved
                        .type_args
                        .iter()
                        .any(|argument| !arena_contains(&self.module.types, *argument))
                {
                    return fail("generic callable arguments do not match its definition");
                }
                let bindings = parameters
                    .iter()
                    .zip(&resolved.type_args)
                    .map(|(parameter, argument)| (parameter.id, *argument))
                    .collect::<Vec<_>>();
                self.specialized_callable_info(generic.function, &bindings)
            }
            Callable::Method(application) => {
                let application = checked_arena(&self.module.method_applications, application)
                    .ok_or_else(|| invalid("callable has an invalid method application"))?;
                self.ordinary_method_info(application.function, application.owner)
            }
            Callable::GenericMethod(application) => {
                let application =
                    checked_arena(&self.module.generic_method_applications, application)
                        .ok_or_else(|| invalid("callable has an invalid generic method"))?;
                let generic = checked_arena(&self.module.generic_methods, application.method)
                    .ok_or_else(|| invalid("generic method has an invalid definition"))?;
                let function = checked_arena(&self.module.functions, generic.function)
                    .ok_or_else(|| invalid("generic method has an invalid function"))?;
                let FunctionGenericity::GenericMethod {
                    definition,
                    owner_parameters,
                    method_parameters,
                } = &function.genericity
                else {
                    return fail("generic method application points to an ordinary function");
                };
                let owner = self.checked_method_owner(
                    Self::ordinary_owner(application.owner),
                    generic.function,
                )?;
                if *definition != application.method
                    || owner_parameters.as_slice() != owner.parameters.as_slice()
                    || method_parameters.len() != application.method_arguments.len()
                    || application
                        .method_arguments
                        .iter()
                        .any(|argument| !arena_contains(&self.module.types, *argument))
                {
                    return fail(
                        "generic method application arguments do not match its definition",
                    );
                }
                let mut bindings = owner_parameters
                    .iter()
                    .zip(owner.arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect::<Vec<_>>();
                bindings.extend(
                    method_parameters
                        .iter()
                        .zip(application.method_arguments.iter())
                        .map(|(parameter, argument)| (parameter.id, *argument)),
                );
                self.validate_method_owner_type(function, &owner, &bindings)?;
                self.specialized_callable_info(generic.function, &bindings)
            }
        }
    }

    fn ordinary_method_info(
        &self,
        function: FunctionId,
        owner: MethodOwnerApplication,
    ) -> Check<CallableInfo> {
        let function_value = checked_arena(&self.module.functions, function)
            .ok_or_else(|| invalid("method application has an invalid function"))?;
        let owner = self.checked_method_owner(owner, function)?;
        let bindings = match &function_value.genericity {
            FunctionGenericity::Plain if owner.parameters.is_empty() => Vec::new(),
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } if owner_parameters.as_slice() == owner.parameters.as_slice() => owner_parameters
                .iter()
                .zip(owner.arguments.iter().copied())
                .map(|(parameter, argument)| (parameter.id, argument))
                .collect(),
            _ => return fail("method application genericity does not match its owner"),
        };
        self.validate_method_owner_type(function_value, &owner, &bindings)?;
        self.specialized_callable_info(function, &bindings)
    }

    fn specialized_callable_info(
        &self,
        function: FunctionId,
        bindings: &[(TypeParamId, TypeId)],
    ) -> Check<CallableInfo> {
        let function_value = checked_arena(&self.module.functions, function)
            .ok_or_else(|| invalid("callable has an invalid function"))?;
        let parameter_types = function_value
            .params
            .iter()
            .map(|parameter| self.instantiate_type(parameter.ty, bindings))
            .collect::<Check<Vec<_>>>()?;
        let receiver = parameter_types.first().copied().map(ReceiverType::Exact);
        Ok(CallableInfo {
            function,
            is_suspend: function_value.is_suspend,
            parameter_types,
            return_ty: self.instantiate_type(function_value.return_ty, bindings)?,
            receiver,
        })
    }

    fn checked_method_owner(
        &self,
        owner: MethodOwnerApplication,
        function: FunctionId,
    ) -> Check<MethodOwnerInfo> {
        let declaration = checked_arena(&self.module.functions, function)
            .ok_or_else(|| invalid("method application has an invalid function"))?;
        let method = declaration
            .method
            .ok_or_else(|| invalid("method application points to a non-method function"))?;
        match owner {
            MethodOwnerApplication::Class(application) => {
                let application = self.checked_class_application(application)?;
                if self
                    .module
                    .objects
                    .iter()
                    .any(|(_, object)| object.backing_class == application.template)
                    || !self.module.classes[application.template]
                        .methods
                        .contains(&function)
                {
                    return fail("method application does not belong to its class owner");
                }
                Ok(MethodOwnerInfo {
                    arguments: application.arguments.clone(),
                    parameters: self.module.classes[application.template]
                        .type_params
                        .clone(),
                    canonical_type: application.canonical_type,
                })
            }
            MethodOwnerApplication::Struct(application) => {
                let application = self.checked_struct_application(application)?;
                if !self.module.structs[application.template]
                    .methods
                    .contains(&function)
                {
                    return fail("method application does not belong to its struct owner");
                }
                Ok(MethodOwnerInfo {
                    arguments: application.arguments.clone(),
                    parameters: self.module.structs[application.template]
                        .type_params
                        .clone(),
                    canonical_type: application.canonical_type,
                })
            }
            MethodOwnerApplication::Enum(application) => {
                let application = self.checked_enum_application(application)?;
                if !self.module.enums[application.template]
                    .methods
                    .contains(&function)
                {
                    return fail("method application does not belong to its enum owner");
                }
                Ok(MethodOwnerInfo {
                    arguments: application.arguments.clone(),
                    parameters: self.module.enums[application.template].type_params.clone(),
                    canonical_type: application.canonical_type,
                })
            }
            MethodOwnerApplication::Interface(application) => {
                let application = self.checked_interface_application(application)?;
                let interface = &self.module.interfaces[application.template];
                let declared_member = interface.methods.iter().find_map(|candidate| {
                    checked_arena(&self.module.interface_methods, *candidate).and_then(|member| {
                        (member.owner == application.template && member.function == function)
                            .then_some(*candidate)
                    })
                });
                let declared = match method.dispatch {
                    MethodDispatch::Interface(member) => declared_member == Some(member),
                    MethodDispatch::Direct => interface.private_methods.contains(&function),
                    MethodDispatch::Virtual(_) | MethodDispatch::FinalOverride(_) => false,
                };
                if !declared {
                    return fail("method application does not belong to its interface owner");
                }
                Ok(MethodOwnerInfo {
                    arguments: application.arguments.clone(),
                    parameters: interface.type_params.clone(),
                    canonical_type: application.canonical_type,
                })
            }
            MethodOwnerApplication::Object(owner) => {
                let object_type = checked_arena(&self.module.object_types, owner)
                    .ok_or_else(|| invalid("method application has an invalid object owner"))?;
                let object = checked_arena(&self.module.objects, object_type.declaration)
                    .ok_or_else(|| invalid("method application object has no declaration"))?;
                let representation = self.checked_class_application(object_type.representation)?;
                if object.object_type != owner
                    || object.backing_class != representation.template
                    || self.module.classes[object.backing_class].self_application
                        != object_type.representation
                    || object_type.canonical_type != representation.canonical_type
                    || !self.module.classes[object.backing_class]
                        .methods
                        .contains(&function)
                {
                    return fail("method application does not belong to its object owner");
                }
                Ok(MethodOwnerInfo {
                    arguments: Vec::new(),
                    parameters: Vec::new(),
                    canonical_type: object_type.canonical_type,
                })
            }
        }
    }

    fn validate_method_owner_type(
        &self,
        function: &Function,
        owner: &MethodOwnerInfo,
        bindings: &[(TypeParamId, TypeId)],
    ) -> Check {
        let method = function
            .method
            .ok_or_else(|| invalid("method application points to a non-method function"))?;
        if self.instantiate_type(method.owner, bindings)? != owner.canonical_type {
            return fail("method application owner differs from the method declaration");
        }
        Ok(())
    }

    const fn ordinary_owner(owner: GenericMethodOwner) -> MethodOwnerApplication {
        match owner {
            GenericMethodOwner::Class(application) => MethodOwnerApplication::Class(application),
            GenericMethodOwner::Struct(application) => MethodOwnerApplication::Struct(application),
            GenericMethodOwner::Enum(application) => MethodOwnerApplication::Enum(application),
            GenericMethodOwner::Object(owner) => MethodOwnerApplication::Object(owner),
        }
    }

    fn callable_method_owner(&self, callable: Callable) -> Check<MethodOwnerApplication> {
        match callable {
            Callable::Method(application) => {
                checked_arena(&self.module.method_applications, application)
                    .map(|application| application.owner)
                    .ok_or_else(|| invalid("bound class call has an invalid method application"))
            }
            Callable::GenericMethod(application) => {
                checked_arena(&self.module.generic_method_applications, application)
                    .map(|application| Self::ordinary_owner(application.owner))
                    .ok_or_else(|| {
                        invalid("bound class call has an invalid generic method application")
                    })
            }
            Callable::Function(_) | Callable::Generic(_) => {
                fail("bound class call does not name a class member")
            }
        }
    }

    fn validate_class_bound_owner(
        &self,
        bound: ClassApplicationId,
        owner: MethodOwnerApplication,
    ) -> Check {
        let MethodOwnerApplication::Class(target) = owner else {
            return fail("bound class call names a non-class method owner");
        };
        let mut current = bound;
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(current.into_raw().into_u32()) {
                return fail("class bound inheritance contains a cycle");
            }
            if current == target {
                return Ok(());
            }
            let application = self.checked_class_application(current)?;
            let declaration = &self.module.classes[application.template];
            let Some(base) = declaration.base_class else {
                return fail("bound class call owner is outside its inheritance chain");
            };
            let bindings = declaration
                .type_params
                .iter()
                .zip(&application.arguments)
                .map(|(parameter, argument)| (parameter.id, *argument))
                .collect::<Vec<_>>();
            let base = self.instantiate_type(base, &bindings)?;
            let Type::Class(base) = self.module.types[base] else {
                return fail("class bound base edge is not a class application");
            };
            current = base;
        }
    }

    fn validate_parameter_interface_bound(
        &self,
        parameter: &TypeParamDecl,
        target: InterfaceApplicationId,
    ) -> Check {
        self.checked_interface_application(target)?;
        let mut roots = parameter
            .interface_bounds()
            .iter()
            .map(|bound| bound.application)
            .collect::<Vec<_>>();
        if let Some(bound) = parameter.class_bound() {
            self.collect_class_interfaces(bound.application, &mut roots, &mut HashSet::new())?;
        }
        let mut closure = Vec::new();
        for root in roots {
            self.append_interface_closure(root, &mut closure)?;
        }
        if !closure.contains(&target) {
            return fail("bound protocol call does not match its interface bound");
        }
        Ok(())
    }

    pub(super) fn checked_function_type(&self, id: FunctionTypeId) -> Check<&FunctionType> {
        let signature = checked_arena(&self.module.function_types, id)
            .ok_or_else(|| invalid("bound protocol call has an invalid signature"))?;
        if !arena_contains(&self.module.types, signature.canonical_type)
            || !matches!(self.module.types[signature.canonical_type], Type::Function(found) if found == id)
            || signature
                .parameter_types
                .iter()
                .any(|ty| !arena_contains(&self.module.types, *ty))
            || !arena_contains(&self.module.types, signature.return_type)
        {
            return fail("bound protocol call signature is not canonical");
        }
        Ok(signature)
    }

    fn bound_signature_matches(&self, signature: &FunctionType, info: &CallableInfo) -> bool {
        signature.is_suspend == info.is_suspend
            && info
                .parameter_types
                .split_first()
                .is_some_and(|(_, parameters)| signature.parameter_types == parameters)
            && signature.return_type == info.return_ty
    }

    pub(in super::super) fn validate_next_callable(
        &self,
        application: MethodApplicationId,
        receiver_ty: TypeId,
        return_ty: TypeId,
    ) -> Check {
        let info = self.callable_info(Callable::Method(application))?;
        if info.is_suspend
            || info.parameter_types.as_slice() != [receiver_ty]
            || info.receiver != Some(ReceiverType::Exact(receiver_ty))
            || info.return_ty != return_ty
        {
            return fail("next callable signature does not match Iterator<E>.next");
        }
        Ok(())
    }
}
