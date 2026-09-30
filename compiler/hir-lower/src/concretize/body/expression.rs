use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_expr(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let previous = self.type_use_site;
        self.type_use_site = self
            .instantiation_site
            .or_else(|| self.current_source_site(source.origin.concrete().evaluation))
            .or(previous);
        let expression = self.lower_expr_at_site(source, substitution, locals);
        self.type_use_site = previous;
        expression
    }

    fn lower_expr_at_site(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let ty = self.lower_type(source.ty, substitution);
        if let Some(location) = self.lower_current_source_location(source, substitution, ty) {
            return location;
        }
        let kind = match &source.kind {
            export::ExprKind::StringLiteral { value, owner } => concrete::ExprKind::StringLiteral {
                value: value.clone(),
                owner: match *owner {
                    export::StringConstantOwner::CurrentDefinition => {
                        export::StringConstantOwner::CurrentDefinition
                    }
                    export::StringConstantOwner::Property(property) => {
                        export::StringConstantOwner::Property(
                            self.source.property_identities[property].property_owner(),
                        )
                    }
                },
            },
            export::ExprKind::IntegerLiteral(value) => concrete::ExprKind::IntegerLiteral(*value),
            export::ExprKind::BoolLiteral(value) => concrete::ExprKind::BoolLiteral(*value),
            export::ExprKind::UnitLiteral => concrete::ExprKind::UnitLiteral,
            export::ExprKind::TupleLiteral(elements) => concrete::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::StructInit { constructor, args } => {
                let constructor =
                    self.lower_struct_constructor_application(*constructor, substitution);
                concrete::ExprKind::StructConstructorCall {
                    constructor,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::StructConstruct {
                application,
                fields,
            } => concrete::ExprKind::StructConstruct {
                struct_id: self.lower_struct_application(*application, substitution),
                fields: fields
                    .iter()
                    .map(|field| self.lower_expr(field, substitution, locals))
                    .collect(),
            },
            export::ExprKind::ClassInit { constructor, args } => {
                let constructor =
                    self.lower_class_constructor_application(*constructor, substitution);
                concrete::ExprKind::ClassNew {
                    constructor,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ConstructorReceiver => concrete::ExprKind::ConstructorReceiver,
            export::ExprKind::ImportedConstructorInit { application, args } => {
                let owner = self.source.imported_constructor_applications[*application].owner;
                let args = args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect();
                match self.source.types[owner] {
                    export::Type::Struct(_) => concrete::ExprKind::StructConstructorCall {
                        constructor: self.lower_imported_struct_constructor_application(
                            *application,
                            substitution,
                        ),
                        args,
                    },
                    export::Type::Class(_) => concrete::ExprKind::ClassNew {
                        constructor: self.lower_imported_class_constructor_application(
                            *application,
                            substitution,
                        ),
                        args,
                    },
                    _ => unreachable!("constructor applications retain their nominal role"),
                }
            }
            export::ExprKind::ConstructorParam(parameter) => concrete::ExprKind::ConstructorParam(
                concrete::ConstructorParamId::from_raw(parameter.into_raw()),
            ),
            export::ExprKind::VariantConstruct { variant, args } => {
                concrete::ExprKind::VariantConstruct {
                    variant: self.lower_enum_variant(*variant, substitution),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::VariantTest { operand, variant } => concrete::ExprKind::VariantTest {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                variant: self.lower_enum_variant(*variant, substitution),
            },
            export::ExprKind::VariantPayloadProject { operand, field } => {
                concrete::ExprKind::VariantPayloadProject {
                    operand: Box::new(self.lower_expr(operand, substitution, locals)),
                    field: self.lower_enum_variant_field(*field, substitution),
                }
            }
            export::ExprKind::Local(local) => {
                concrete::ExprKind::Local(self.lower_local(*local, locals))
            }
            export::ExprKind::GlobalRead(global) => {
                concrete::ExprKind::GlobalRead(self.global_map[global])
            }
            export::ExprKind::GenericDelegateStorageRead(reference) => {
                let specialization = self.request_generic_delegate(reference, substitution);
                concrete::ExprKind::GlobalRead(
                    self.generic_delegate_specializations[specialization].storage,
                )
            }
            export::ExprKind::SingletonValue(value) => {
                concrete::ExprKind::SingletonValue(self.lower_singleton_value(*value))
            }
            export::ExprKind::ImportedSingletonValue(value) => {
                concrete::ExprKind::ImportedSingletonValue(*value)
            }
            export::ExprKind::Capture(binding) => {
                concrete::ExprKind::Capture(concrete::BindingId::from_raw(binding.into_raw()))
            }
            export::ExprKind::ImportedClosure(closure) => {
                self.lower_imported_closure(closure, source.span, substitution, locals)
            }
            export::ExprKind::ImportedMethodCall {
                receiver,
                callee,
                args,
            } => self.lower_imported_method_call(receiver, callee, args, substitution, locals),
            export::ExprKind::ImportedCallableReference(reference) => {
                concrete::ExprKind::CallableReference(self.lower_imported_reference(
                    reference,
                    source.span,
                    substitution,
                    locals,
                ))
            }
            export::ExprKind::Lambda(id) => {
                concrete::ExprKind::Lambda(self.ensure_lambda(*id, substitution, locals))
            }
            export::ExprKind::AnonymousFunction(id) => concrete::ExprKind::AnonymousFunction(
                self.ensure_anonymous(*id, substitution, locals),
            ),
            export::ExprKind::CallableReference(id) => concrete::ExprKind::CallableReference(
                self.ensure_reference(*id, substitution, locals),
            ),
            export::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => concrete::ExprKind::FunctionCoercion {
                source: Box::new(self.lower_expr(source, substitution, locals)),
                coercion: self.ensure_coercion(*coercion, substitution),
                target_type: self.lower_function_type(*target_type, substitution),
            },
            export::ExprKind::PtrFromNonZeroULong(value) => {
                concrete::ExprKind::PtrFromNonZeroULong(Box::new(self.lower_expr(
                    value,
                    substitution,
                    locals,
                )))
            }
            export::ExprKind::PtrToULong(value) => concrete::ExprKind::PtrToULong(Box::new(
                self.lower_expr(value, substitution, locals),
            )),
            export::ExprKind::PtrCast(value) => {
                concrete::ExprKind::PtrCast(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::PtrLoad { pointer, offset } => concrete::ExprKind::PtrLoad {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, substitution, locals))),
            },
            export::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => concrete::ExprKind::PtrStore {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, substitution, locals))),
                value: Box::new(self.lower_expr(value, substitution, locals)),
            },
            export::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => concrete::ExprKind::PtrOffset {
                pointer: Box::new(self.lower_expr(pointer, substitution, locals)),
                offset: Box::new(self.lower_expr(offset, substitution, locals)),
                subtract: *subtract,
            },
            export::ExprKind::AddressOf(place) => {
                concrete::ExprKind::AddressOf(self.lower_place(*place, locals))
            }
            export::ExprKind::SizeOf(size) => {
                concrete::ExprKind::SizeOf(self.lower_type(*size, substitution))
            }
            export::ExprKind::AlignOf(align) => {
                concrete::ExprKind::AlignOf(self.lower_type(*align, substitution))
            }
            export::ExprKind::FunctionAddress(function) => {
                concrete::ExprKind::FunctionAddress(self.request_function(*function, Vec::new()))
            }
            export::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let concrete::TypeKind::Struct(callback) = self.types[ty].kind else {
                    panic!("foreign callback registration has a concrete callback struct type")
                };
                let registration =
                    self.ensure_foreign_callback(*registration, substitution, callback);
                concrete::ExprKind::ForeignCallbackRegister {
                    registration,
                    closure: Box::new(self.lower_expr(closure, substitution, locals)),
                }
            }
            export::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => concrete::ExprKind::ForeignCallbackOperation {
                operation: match operation {
                    export::ForeignCallbackOperation::Retain => {
                        concrete::ForeignCallbackOperation::Retain
                    }
                    export::ForeignCallbackOperation::Release => {
                        concrete::ForeignCallbackOperation::Release
                    }
                    export::ForeignCallbackOperation::State => {
                        concrete::ForeignCallbackOperation::State
                    }
                    export::ForeignCallbackOperation::Failure => {
                        concrete::ForeignCallbackOperation::Failure
                    }
                },
                callback: Box::new(self.lower_expr(callback, substitution, locals)),
            },
            export::ExprKind::FieldAccess { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                }
            }
            export::ExprKind::InitializingClassFieldAccess { field } => {
                let (receiver_ty, field) =
                    self.lower_initializing_class_field(*field, substitution);
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(concrete::Expr {
                        kind: concrete::ExprKind::ConstructorReceiver,
                        ty: receiver_ty,
                        span: source.span,
                        origin: source.origin.concrete(),
                    }),
                    field,
                }
            }
            export::ExprKind::InitializingStructFieldAccess { owner, field } => {
                let receiver_ty = self.lower_type(*owner, substitution);
                let field = self.lower_field_ref(
                    export::FieldRef::StructField {
                        owner: *owner,
                        field: *field,
                    },
                    substitution,
                );
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(concrete::Expr {
                        kind: concrete::ExprKind::ConstructorReceiver,
                        ty: receiver_ty,
                        span: source.span,
                        origin: source.origin.concrete(),
                    }),
                    field,
                }
            }
            export::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                let source_function = self.source.callable_function(*callee);
                assert!(
                    !matches!(
                        &self.source.functions[source_function].kind,
                        export::FunctionKind::Intrinsic(export::IntrinsicFunction {
                            kind: export::IntrinsicFunctionKind::Integer(_),
                            ..
                        })
                    ),
                    "integer intrinsic MethodCall must be normalized before LocalConcrete HIR"
                );
                let mut receiver = self.lower_expr(receiver, substitution, locals);
                let callee = match callee {
                    export::MethodCallee::Callable(callable) => {
                        self.lower_callable(*callable, substitution)
                    }
                    export::MethodCallee::Bound(bound) => {
                        let (callee, interface) =
                            self.resolve_bound_callee(*bound, receiver.ty, substitution);
                        if let Some(interface) = interface {
                            receiver = self.adapt_receiver_to_interface(receiver, interface);
                        }
                        callee
                    }
                    export::MethodCallee::DerivedEquality(application) => {
                        self.lower_derived_equality_application(*application, substitution)
                    }
                };
                concrete::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let callee = match callee {
                    export::MethodCallee::Callable(callable) => {
                        self.lower_callable(*callable, substitution)
                    }
                    export::MethodCallee::Bound(_) => {
                        unreachable!("super resolution never produces a bound interface target")
                    }
                    export::MethodCallee::DerivedEquality(_) => {
                        unreachable!("super resolution only produces declared class methods")
                    }
                };
                concrete::ExprKind::DirectSuperMethodCall {
                    receiver: Box::new(receiver),
                    callee,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::Box(value) => {
                let value = self.lower_expr(value, substitution, locals);
                self.ensure_box_source(value.ty);
                if matches!(
                    self.types[value.ty].kind,
                    concrete::TypeKind::Unit
                        | concrete::TypeKind::Integer(_)
                        | concrete::TypeKind::Boolean
                        | concrete::TypeKind::Struct(_)
                        | concrete::TypeKind::Enum(_)
                        | concrete::TypeKind::Tuple(_)
                        | concrete::TypeKind::Ptr(_)
                        | concrete::TypeKind::FunPtr(_)
                ) {
                    concrete::ExprKind::Box(Box::new(value))
                } else {
                    // A source type parameter with interface-only bounds is
                    // conservatively represented as Box in Export HIR. Its
                    // concrete argument may instead be a reference; in that
                    // case the adaptation is a zero-cost retype.
                    value.kind
                }
            }
            export::ExprKind::Unbox(value) => {
                concrete::ExprKind::Unbox(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::ReferenceUpcast(value) => concrete::ExprKind::ReferenceUpcast(
                Box::new(self.lower_expr(value, substitution, locals)),
            ),
            export::ExprKind::IsInstance { operand, check_ty } => concrete::ExprKind::IsInstance {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                check_ty: self.lower_type(*check_ty, substitution),
            },
            export::ExprKind::Cast {
                operand,
                check_ty,
                optional,
            } => {
                if !optional {
                    self.lower_cast_exception_type();
                }
                concrete::ExprKind::Cast {
                    operand: Box::new(self.lower_expr(operand, substitution, locals)),
                    check_ty: self.lower_type(*check_ty, substitution),
                    optional: *optional,
                }
            }
            export::ExprKind::ArrayLiteral(elements) => concrete::ExprKind::ArrayLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::ArrayAssembly(assembly) => {
                let result = self.lower_type(assembly.result_type, substitution);
                let concrete::TypeKind::Class(result_type) = self.types[result].kind else {
                    unreachable!("an array assembly retains its exact class type")
                };
                concrete::ExprKind::ArrayAssembly(concrete::ArrayAssembly {
                    element_type: self.lower_type(assembly.element_type, substitution),
                    parts: assembly
                        .parts
                        .iter()
                        .map(|part| match part {
                            export::ArrayAssemblyPart::Element(value) => {
                                concrete::ArrayAssemblyPart::Element(self.lower_expr(
                                    value,
                                    substitution,
                                    locals,
                                ))
                            }
                            export::ArrayAssemblyPart::CopyArray(value) => {
                                concrete::ArrayAssemblyPart::CopyArray(self.lower_expr(
                                    value,
                                    substitution,
                                    locals,
                                ))
                            }
                        })
                        .collect(),
                    result_type,
                })
            }
            export::ExprKind::Index {
                access,
                receiver,
                index,
            } => {
                self.lower_array_bounds_exception_type();
                concrete::ExprKind::Index {
                    access: *access,
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    index: Box::new(self.lower_expr(index, substitution, locals)),
                }
            }
            export::ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => {
                self.lower_array_bounds_exception_type();
                concrete::ExprKind::ArraySet {
                    access: *access,
                    receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                    index: Box::new(self.lower_expr(index, substitution, locals)),
                    value: Box::new(self.lower_expr(value, substitution, locals)),
                }
            }
            export::ExprKind::ArrayLen(array) => {
                concrete::ExprKind::ArrayLen(Box::new(self.lower_expr(array, substitution, locals)))
            }
            export::ExprKind::ArrayClone(array) => concrete::ExprKind::ArrayClone(Box::new(
                self.lower_expr(array, substitution, locals),
            )),
            export::ExprKind::Call {
                callee,
                args,
                receiver,
            } => concrete::ExprKind::Call {
                binding: None,
                receiver: receiver.map(|ty| self.lower_type(ty, substitution)),
                callee: concrete::CallableTarget::Local(self.lower_callable(*callee, substitution)),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },

            export::ExprKind::ImportedGenericCall {
                application,
                kind: call_kind,
                args,
                receiver,
                binding,
            } => {
                let application = &self.source.imported_generic_applications[*application];
                let callee = self.lower_imported_callable_application(application, substitution);
                let args: Vec<_> = args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect();
                let receiver = receiver.map(|ty| self.lower_type(ty, substitution));
                if matches!(
                    application.arguments,
                    export::ImportedCallableArguments::Method { .. }
                ) {
                    let mut args = args.into_iter();
                    let receiver =
                        Box::new(args.next().expect("a member call has a receiver argument"));
                    let args = args.collect();
                    let callee = concrete::Callable::Function(callee);
                    match call_kind {
                        export::ImportedGenericCallKind::Ordinary => {
                            concrete::ExprKind::MethodCall {
                                receiver,
                                callee,
                                args,
                            }
                        }
                        export::ImportedGenericCallKind::DirectSuper => {
                            concrete::ExprKind::DirectSuperMethodCall {
                                receiver,
                                callee,
                                args,
                            }
                        }
                    }
                } else {
                    concrete::ExprKind::Call {
                        callee: concrete::CallableTarget::Local(concrete::Callable::Function(
                            callee,
                        )),
                        binding: binding.clone(),
                        receiver,
                        args,
                    }
                }
            }
            export::ExprKind::ImportedDependencyCall {
                callee,
                binding,
                args,
                receiver,
            } => concrete::ExprKind::Call {
                receiver: receiver.map(|ty| self.lower_type(ty, substitution)),
                callee: concrete::CallableTarget::Imported(
                    self.imported_dependency_callable_map[callee],
                ),
                binding: binding.clone(),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => {
                let (callee, function_arguments) =
                    self.lower_callable_with_arguments(*callee, substitution);
                concrete::ExprKind::LocalFunctionCall {
                    local_function: self
                        .ensure_local_function(*local_function, &function_arguments),
                    callee,
                    captures: captures
                        .iter()
                        .map(|capture| self.lower_expr(capture, substitution, locals))
                        .collect(),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => concrete::ExprKind::CallableCall {
                callee: Box::new(self.lower_expr(callee, substitution, locals)),
                function_type: self.lower_function_type(*function_type, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::PrimitiveBinary { kind, lhs, rhs } => {
                concrete::ExprKind::PrimitiveBinary {
                    kind: *kind,
                    lhs: Box::new(self.lower_expr(lhs, substitution, locals)),
                    rhs: Box::new(self.lower_expr(rhs, substitution, locals)),
                }
            }
            export::ExprKind::PrimitiveUnary { kind, operand } => {
                concrete::ExprKind::PrimitiveUnary {
                    kind: *kind,
                    operand: Box::new(self.lower_expr(operand, substitution, locals)),
                }
            }
            export::ExprKind::IntegerOperation {
                operation,
                arguments,
            } => {
                let operation = *operation;
                if matches!(operation, concrete::IntegerOperation::Managed { .. }) {
                    self.lower_arithmetic_exception_type();
                }
                let arguments =
                    match arguments {
                        export::HirIntegerOperationArguments::Unary(operand) => {
                            concrete::HirIntegerOperationArguments::Unary(Box::new(
                                self.lower_expr(operand, substitution, locals),
                            ))
                        }
                        export::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                            concrete::HirIntegerOperationArguments::Binary {
                                lhs: Box::new(self.lower_expr(lhs, substitution, locals)),
                                rhs: Box::new(self.lower_expr(rhs, substitution, locals)),
                            }
                        }
                    };
                concrete::ExprKind::IntegerOperation {
                    operation,
                    arguments,
                }
            }
            export::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => concrete::ExprKind::IntegerConversion {
                conversion: *conversion,
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
            },
            export::ExprKind::Binary { op, lhs, rhs } => concrete::ExprKind::Binary {
                op: *op,
                lhs: Box::new(self.lower_expr(lhs, substitution, locals)),
                rhs: Box::new(self.lower_expr(rhs, substitution, locals)),
            },
            export::ExprKind::Unary { op, operand } => concrete::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
            },
            export::ExprKind::SomeWrap(value) => {
                concrete::ExprKind::SomeWrap(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::NoneLiteral => concrete::ExprKind::NoneLiteral,
            export::ExprKind::IsSome(value) => {
                concrete::ExprKind::IsSome(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                if *trap_on_none {
                    self.lower_unwrap_exception_type();
                }
                concrete::ExprKind::Unwrap {
                    operand: Box::new(self.lower_expr(operand, substitution, locals)),
                    trap_on_none: *trap_on_none,
                }
            }
        };
        concrete::Expr {
            kind,
            ty,
            span: source.span,
            origin: source.origin.concrete(),
        }
    }
}
