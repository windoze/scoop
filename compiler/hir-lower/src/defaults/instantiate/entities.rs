use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_imported_method_callee(
        &mut self,
        source: &hir::ImportedMethodCallee,
        origin: hir::ExpressionOrigin,
        context: &InstantiationContext,
    ) -> hir::ImportedMethodCallee {
        match source {
            hir::ImportedMethodCallee::Callable(callee) => hir::ImportedMethodCallee::Callable(
                self.instantiate_default_callable_target(*callee, context),
            ),
            hir::ImportedMethodCallee::InterfaceBound(bound) => {
                hir::ImportedMethodCallee::InterfaceBound(Box::new(
                    hir::ImportedInterfaceBoundCallable {
                        receiver_type: self
                            .instantiate_method_ty(bound.receiver_type, &context.bindings),
                        interface: self.instantiate_method_ty(bound.interface, &context.bindings),
                        member: bound.member,
                        slot: bound.slot,
                        declared: self.instantiate_default_callable_target(bound.declared, context),
                        signature: self.instantiate_default_function_type(bound.signature, context),
                    },
                ))
            }
            hir::ImportedMethodCallee::DerivedEquality(application) => {
                let hir::MethodCallee::DerivedEquality(application) = self
                    .instantiate_default_method_callee(
                        hir::MethodCallee::DerivedEquality(*application),
                        origin,
                        context,
                    )
                else {
                    unreachable!("an equality application retains its callable kind")
                };
                hir::ImportedMethodCallee::DerivedEquality(application)
            }
        }
    }

    pub(super) fn instantiate_default_initializing_field(
        &mut self,
        source: hir::InitializingClassFieldRef,
        context: &mut InstantiationContext,
    ) -> hir::InitializingClassFieldRef {
        hir::InitializingClassFieldRef {
            owner: self.instantiate_method_ty(source.owner, &context.bindings),
            field: source.field,
        }
    }

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
        origin: hir::ExpressionOrigin,
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
                        .derived_equality_candidate_at(ty, origin)
                        .expect("a validated default keeps a valid equality derivation")
                        .expect("the original expression has a derived equality target");
                    let application = match candidate {
                        crate::derived::DerivedEqualityCandidate::Nominal {
                            application, ..
                        }
                        | crate::derived::DerivedEqualityCandidate::TypeOwned {
                            application, ..
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
        self.intern_struct_application(source.template, arguments)
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
        self.intern_enum_application(source.template, arguments)
    }

    pub(super) fn instantiate_default_variant(
        &mut self,
        source: hir::EnumVariantApplication,
        context: &InstantiationContext,
    ) -> hir::EnumVariantApplication {
        hir::EnumVariantApplication {
            owner: self.instantiate_method_ty(source.owner, &context.bindings),
            variant: source.variant,
        }
    }

    pub(super) fn instantiate_default_enum_field(
        &mut self,
        source: hir::EnumVariantFieldApplication,
        context: &InstantiationContext,
    ) -> hir::EnumVariantFieldApplication {
        hir::EnumVariantFieldApplication {
            variant: self.instantiate_default_variant(source.variant, context),
            field: source.field,
        }
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
        self.intern_class_application(source.template, arguments)
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
        self.intern_interface_application(source.template, arguments)
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
            hir::FieldRef::StructField { owner, field } => hir::FieldRef::StructField {
                owner: self.instantiate_method_ty(owner, &context.bindings),
                field,
            },
            hir::FieldRef::ClassField { owner, field } => hir::FieldRef::ClassField {
                owner: self.instantiate_method_ty(owner, &context.bindings),
                field,
            },
            hir::FieldRef::TupleIndex(index) => hir::FieldRef::TupleIndex(index),
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
        self.function_coercion(source, target)
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
                span: source.span,
            })
    }
}
