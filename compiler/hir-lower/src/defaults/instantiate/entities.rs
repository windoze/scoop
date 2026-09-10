use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_callable(
        &mut self,
        source: hir::Callable,
        context: &InstantiationContext,
    ) -> hir::Callable {
        match source {
            hir::Callable::Function(function) => hir::Callable::Function(function),
            hir::Callable::Generic(application) => {
                let application = self.instantiations[application].clone();
                let function = self.generic_functions[application.generic].function;
                let arguments = application
                    .type_args
                    .into_iter()
                    .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
                    .collect();
                hir::Callable::Generic(self.record_instantiation(function, arguments))
            }
            hir::Callable::Method(application) => {
                let application = self.method_applications[application].clone();
                let owner = self.instantiate_default_method_owner(application.owner, context);
                hir::Callable::Method(self.record_method_application(application.function, owner))
            }
            hir::Callable::GenericMethod(application) => {
                let application = self.generic_method_applications[application].clone();
                let function = self.generic_methods[application.method].function;
                let owner = match application.owner {
                    hir::GenericMethodOwner::Class(owner) => hir::GenericMethodOwner::Class(
                        self.instantiate_default_class_application(owner, context),
                    ),
                    hir::GenericMethodOwner::Struct(owner) => hir::GenericMethodOwner::Struct(
                        self.instantiate_default_struct_application(owner, context),
                    ),
                    hir::GenericMethodOwner::Enum(owner) => hir::GenericMethodOwner::Enum(
                        self.instantiate_default_enum_application(owner, context),
                    ),
                    hir::GenericMethodOwner::Object(owner) => {
                        hir::GenericMethodOwner::Object(owner)
                    }
                };
                let arguments = application
                    .method_arguments
                    .iter()
                    .map(|ty| self.instantiate_method_ty(*ty, &context.bindings))
                    .collect();
                hir::Callable::GenericMethod(
                    self.record_generic_method_application(function, owner, arguments),
                )
            }
        }
    }

    pub(super) fn instantiate_default_method_callee(
        &mut self,
        source: hir::MethodCallee,
        context: &InstantiationContext,
    ) -> hir::MethodCallee {
        match source {
            hir::MethodCallee::Callable(callable) => {
                hir::MethodCallee::Callable(self.instantiate_default_callable(callable, context))
            }
            hir::MethodCallee::Bound(bound) => {
                let source = self.bound_callable_refs[bound].clone();
                let bound_source = match source.source {
                    hir::BoundCallableSource::Class { bound, callable } => {
                        hir::BoundCallableSource::Class {
                            bound: self.instantiate_default_class_application(bound, context),
                            callable: self.instantiate_default_callable(callable, context),
                        }
                    }
                    hir::BoundCallableSource::Interface { bound, member } => {
                        hir::BoundCallableSource::Interface {
                            bound: self.instantiate_default_interface_application(bound, context),
                            member,
                        }
                    }
                };
                let instantiated_signature =
                    self.instantiate_default_function_type(source.instantiated_signature, context);
                let value = hir::BoundCallableRef {
                    receiver_parameter: source.receiver_parameter,
                    source: bound_source,
                    instantiated_signature,
                };
                let existing = self
                    .bound_callable_refs
                    .iter()
                    .find_map(|(id, existing)| (existing == &value).then_some(id));
                let id = existing.unwrap_or_else(|| self.bound_callable_refs.alloc(value));
                hir::MethodCallee::Bound(id)
            }
            hir::MethodCallee::DerivedEquality(application) => {
                let source = self.derived_equality_applications[application].clone();
                let ty = self.instantiate_method_ty(source.owner_ty, &context.bindings);
                if ty == source.owner_ty {
                    hir::MethodCallee::DerivedEquality(application)
                } else {
                    let candidate = self
                        .derived_equality_candidate(ty, source.span)
                        .expect("a validated default keeps a valid equality derivation")
                        .expect("the original expression has a derived equality target");
                    let application = match candidate {
                        crate::derived::DerivedEqualityCandidate::Nominal {
                            application, ..
                        }
                        | crate::derived::DerivedEqualityCandidate::Structural {
                            application,
                            ..
                        } => application,
                    };
                    hir::MethodCallee::DerivedEquality(application)
                }
            }
        }
    }

    pub(super) fn instantiate_default_method_owner(
        &mut self,
        source: hir::MethodOwnerApplication,
        context: &InstantiationContext,
    ) -> hir::MethodOwnerApplication {
        match source {
            hir::MethodOwnerApplication::Class(application) => hir::MethodOwnerApplication::Class(
                self.instantiate_default_class_application(application, context),
            ),
            hir::MethodOwnerApplication::Struct(application) => {
                hir::MethodOwnerApplication::Struct(
                    self.instantiate_default_struct_application(application, context),
                )
            }
            hir::MethodOwnerApplication::Enum(application) => hir::MethodOwnerApplication::Enum(
                self.instantiate_default_enum_application(application, context),
            ),
            hir::MethodOwnerApplication::Interface(application) => {
                hir::MethodOwnerApplication::Interface(
                    self.instantiate_default_interface_application(application, context),
                )
            }
            hir::MethodOwnerApplication::Object(owner) => {
                hir::MethodOwnerApplication::Object(owner)
            }
        }
    }

    pub(super) fn instantiate_default_struct_application(
        &mut self,
        source: hir::StructApplicationId,
        context: &InstantiationContext,
    ) -> hir::StructApplicationId {
        let source = self.struct_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.struct_application_id(source.template, arguments)
    }

    pub(super) fn instantiate_default_enum_application(
        &mut self,
        source: hir::EnumApplicationId,
        context: &InstantiationContext,
    ) -> hir::EnumApplicationId {
        let source = self.enum_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.enum_application_id(source.template, arguments)
    }

    pub(super) fn instantiate_default_applied_enum_variant(
        &mut self,
        source: hir::AppliedEnumVariantRef,
        context: &InstantiationContext,
    ) -> hir::AppliedEnumVariantRef {
        let application = self.instantiate_default_enum_application(source.application(), context);
        hir::AppliedEnumVariantRef::checked(
            &self.enums,
            &self.enum_applications,
            application,
            source.declaration(),
        )
        .expect("default substitution preserves the enum variant template")
    }

    pub(super) fn instantiate_default_applied_enum_field(
        &mut self,
        source: hir::AppliedEnumVariantFieldRef,
        context: &InstantiationContext,
    ) -> hir::AppliedEnumVariantFieldRef {
        let variant = self.instantiate_default_applied_enum_variant(source.variant(), context);
        hir::AppliedEnumVariantFieldRef::checked(
            &self.enums,
            &self.enum_applications,
            variant,
            source.local_index(),
        )
        .expect("default substitution preserves the enum payload field")
    }

    pub(super) fn instantiate_default_class_application(
        &mut self,
        source: hir::ClassApplicationId,
        context: &InstantiationContext,
    ) -> hir::ClassApplicationId {
        let source = self.class_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.class_application_id(source.template, arguments)
    }

    pub(super) fn instantiate_default_class_constructor(
        &mut self,
        source: hir::ClassConstructorApplicationId,
        context: &InstantiationContext,
    ) -> hir::ClassConstructorApplicationId {
        let source = self.class_constructor_applications[source].clone();
        let owner = self.instantiate_default_class_application(source.owner, context);
        self.class_constructor_application(source.constructor, owner)
    }

    pub(super) fn instantiate_default_struct_constructor(
        &mut self,
        source: hir::StructConstructorApplicationId,
        context: &InstantiationContext,
    ) -> hir::StructConstructorApplicationId {
        let source = self.struct_constructor_applications[source].clone();
        let owner = self.instantiate_default_struct_application(source.owner, context);
        self.struct_constructor_application(source.constructor, owner)
    }

    pub(super) fn instantiate_default_interface_application(
        &mut self,
        source: hir::InterfaceApplicationId,
        context: &InstantiationContext,
    ) -> hir::InterfaceApplicationId {
        let source = self.interface_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.interface_application_id(source.template, arguments)
    }

    pub(super) fn instantiate_default_function_type(
        &mut self,
        source: hir::FunctionTypeId,
        context: &InstantiationContext,
    ) -> hir::FunctionTypeId {
        let canonical = self.function_types[source].canonical_type;
        let ty = self.instantiate_method_ty(canonical, &context.bindings);
        let Type::Function(function) = self.types[ty] else {
            unreachable!("function type substitution remains a function type")
        };
        function
    }

    pub(super) fn instantiate_default_field(
        &mut self,
        source: hir::FieldRef,
        context: &InstantiationContext,
    ) -> hir::FieldRef {
        match source {
            hir::FieldRef::StructField(field) => {
                let application =
                    self.instantiate_default_struct_application(field.application(), context);
                let field = hir::AppliedStructFieldRef::checked(
                    &self.structs,
                    &self.struct_applications,
                    application,
                    field.local_index(),
                )
                .expect("default substitution preserves a checked struct field");
                hir::FieldRef::StructField(field)
            }
            hir::FieldRef::TupleIndex(index) => hir::FieldRef::TupleIndex(index),
            hir::FieldRef::ClassField { application, field } => hir::FieldRef::ClassField {
                application: self.instantiate_default_class_application(application, context),
                field,
            },
        }
    }

    pub(super) fn instantiate_default_coercion(
        &mut self,
        source: hir::FunctionCoercionId,
        context: &InstantiationContext,
    ) -> hir::FunctionCoercionId {
        let value = self.function_coercions[source].clone();
        let source = self.instantiate_default_function_type(value.source, context);
        let target = self.instantiate_default_function_type(value.target, context);
        if let Some(&coercion) = self.function_coercion_by_types.get(&(source, target)) {
            return coercion;
        }
        let coercion = self
            .function_coercions
            .alloc(hir::FunctionCoercion { source, target });
        self.function_coercion_by_types
            .insert((source, target), coercion);
        coercion
    }

    pub(super) fn instantiate_default_foreign_callback(
        &mut self,
        source: hir::ForeignCallbackRegistrationId,
        context: &InstantiationContext,
    ) -> hir::ForeignCallbackRegistrationId {
        let source = self.foreign_callback_registrations[source].clone();
        let native_function_type =
            self.instantiate_default_function_type(source.native_function_type, context);
        let managed_function_type =
            self.instantiate_default_function_type(source.managed_function_type, context);
        self.foreign_callback_registrations
            .alloc(hir::ForeignCallbackRegistration {
                definition_root: source.definition_root,
                definition_path: source.definition_path,
                native_function_type,
                managed_function_type,
                context_index: source.context_index,
                mode: source.mode,
            })
    }

    pub(super) fn instantiate_default_lambda(
        &mut self,
        source: hir::LambdaId,
        context: &mut InstantiationContext,
    ) -> hir::LambdaId {
        let source = self.lambdas[source].clone();
        let body_type_arguments =
            self.instantiate_callable_body_arguments(source.function, context);
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.lambdas.alloc(hir::Lambda {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_anonymous(
        &mut self,
        source: hir::AnonymousFunctionId,
        context: &mut InstantiationContext,
    ) -> hir::AnonymousFunctionId {
        let source = self.anonymous_functions[source].clone();
        let body_type_arguments =
            self.instantiate_callable_body_arguments(source.function, context);
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.anonymous_functions.alloc(hir::AnonymousFunction {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_reference(
        &mut self,
        source: hir::CallableReferenceId,
        context: &mut InstantiationContext,
    ) -> hir::CallableReferenceId {
        let source = self.callable_references[source].clone();
        let target = match source.target {
            hir::CallableReferenceTarget::Named(callee) => hir::CallableReferenceTarget::Named(
                self.instantiate_default_callable(callee, context),
            ),
            hir::CallableReferenceTarget::Local {
                local_function,
                callee,
            } => hir::CallableReferenceTarget::Local {
                local_function,
                callee: self.instantiate_default_callable(callee, context),
            },
            hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                hir::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_method_callee(callee, context),
                }
            }
            hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                hir::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_callable(callee, context),
                }
            }
        };
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.callable_references.alloc(hir::CallableReference {
            definition_root: source.definition_root,
            definition_path: source.definition_path,
            target,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            captures,
            span: source.span,
        })
    }

    pub(super) fn instantiate_default_capture(
        &mut self,
        source: &hir::Capture,
        context: &mut InstantiationContext,
    ) -> hir::Capture {
        hir::Capture {
            binding: source.binding,
            name: source.name.clone(),
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            first_use_span: source.first_use_span,
            source: self.instantiate_default_expr(&source.source, context),
        }
    }

    pub(super) fn instantiate_callable_body_arguments(
        &mut self,
        function: hir::FunctionId,
        context: &InstantiationContext,
    ) -> hir::CallableBodyTypeArguments {
        let parameters = self.functions[function]
            .type_params()
            .into_iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let arguments = parameters
            .into_iter()
            .map(|parameter| {
                let ty = self.intern_type(Type::Param(parameter));
                self.instantiate_method_ty(ty, &context.bindings)
            })
            .collect();
        hir::CallableBodyTypeArguments::Explicit(arguments)
    }
}
