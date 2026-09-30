use super::*;

impl Lowerer {
    pub(in crate::effects) fn generic_call(
        &self,
        callable: hir::Callable,
        span: Span,
    ) -> Option<GenericCall> {
        let (callee, arguments) = match callable {
            hir::Callable::Function(_) => return None,
            hir::Callable::Generic(application) => {
                let application = &self.instantiations[application];
                let generic = &self.generic_functions[application.generic];
                let hir::FunctionGenericity::Generic { parameters, .. } =
                    &self.functions[generic.function].genericity
                else {
                    unreachable!("a generic application names a generic function declaration")
                };
                assert_eq!(parameters.len(), application.type_args.len());
                let arguments = parameters
                    .iter()
                    .zip(application.type_args.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (generic.function, arguments)
            }
            hir::Callable::Method(application) => {
                let application = &self.method_applications[application];
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters, ..
                } = &self.functions[application.function].genericity
                else {
                    // Parameter-free ordinary methods have no generic GC-free
                    // preconditions and therefore need no call-site record.
                    return None;
                };
                let owner_arguments = self.method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                let arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (application.function, arguments)
            }
            hir::Callable::GenericMethod(application) => {
                let application = &self.generic_method_applications[application];
                let method = &self.generic_methods[application.method];
                let hir::FunctionGenericity::GenericMethod {
                    owner_parameters,
                    method_parameters,
                    ..
                } = &self.functions[method.function].genericity
                else {
                    unreachable!("a generic method application names a generic method declaration")
                };
                let owner_arguments = self.generic_method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                assert_eq!(method_parameters.len(), application.method_arguments.len());
                let mut arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect::<Vec<_>>();
                arguments.extend(
                    method_parameters
                        .iter()
                        .zip(application.method_arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument)),
                );
                (method.function, arguments)
            }
        };
        Some(GenericCall {
            callee: GenericCallable::Function(callee),
            arguments,
            span,
        })
    }

    pub(in crate::effects) fn callable_body_generic_call(
        &self,
        definition: hir::LexicalFunctionDefinition,
        body_type_arguments: &hir::CallableBodyTypeArguments,
        span: Span,
    ) -> Option<GenericCall> {
        let parameters = self.lexical_body_type_parameters(definition);
        if parameters.is_empty() {
            return None;
        }
        let argument_types = match body_type_arguments {
            hir::CallableBodyTypeArguments::Lexical => parameters
                .iter()
                .map(|parameter| {
                    self.types
                        .iter()
                        .find_map(|(ty, candidate)| {
                            matches!(candidate, hir::Type::Param(id) if *id == *parameter)
                                .then_some(ty)
                        })
                        .expect("every callable body parameter has a canonical parameter type")
                })
                .collect::<Vec<_>>(),
            hir::CallableBodyTypeArguments::Explicit(arguments) => arguments.clone(),
        };
        assert_eq!(parameters.len(), argument_types.len());
        Some(GenericCall {
            callee: match definition {
                hir::LexicalFunctionDefinition::Source { function, .. } => {
                    GenericCallable::Function(function)
                }
                hir::LexicalFunctionDefinition::Template(template) => {
                    GenericCallable::Imported(template)
                }
            },
            arguments: parameters.into_iter().zip(argument_types).collect(),
            span,
        })
    }

    fn generic_method_owner_arguments(&self, owner: hir::GenericMethodOwner) -> &[hir::TypeId] {
        match owner {
            hir::GenericMethodOwner::Class(id) => &self.class_applications[id].arguments,
            hir::GenericMethodOwner::Struct(id) => &self.struct_applications[id].arguments,
            hir::GenericMethodOwner::Enum(id) => &self.enum_applications[id].arguments,
            hir::GenericMethodOwner::Object(_) => &[],
        }
    }

    pub(in crate::effects) fn generic_class_constructor_call(
        &self,
        id: hir::ClassConstructorApplicationId,
        span: Span,
    ) -> Option<GenericCall> {
        let application = &self.class_constructor_applications[id];
        let owner = &self.class_applications[application.owner];
        self.generic_constructor_call(
            GenericCallable::ClassConstructor(application.constructor),
            &owner.arguments,
            span,
        )
    }

    pub(in crate::effects) fn generic_struct_constructor_call(
        &self,
        id: hir::StructConstructorApplicationId,
        span: Span,
    ) -> Option<GenericCall> {
        let application = &self.struct_constructor_applications[id];
        let owner = &self.struct_applications[application.owner];
        self.generic_constructor_call(
            GenericCallable::StructConstructor(application.constructor),
            &owner.arguments,
            span,
        )
    }

    fn generic_constructor_call(
        &self,
        callee: GenericCallable,
        arguments: &[hir::TypeId],
        span: Span,
    ) -> Option<GenericCall> {
        let parameters = self.effect_callable_parameters(callee);
        assert_eq!(parameters.len(), arguments.len());
        if parameters.is_empty() {
            return None;
        }
        Some(GenericCall {
            callee,
            arguments: parameters
                .iter()
                .zip(arguments)
                .map(|(p, a)| (p.id, *a))
                .collect(),
            span,
        })
    }
}
