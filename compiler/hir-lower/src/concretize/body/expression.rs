use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_expr(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let ty = self.lower_type(source.ty, substitution);
        if let Some(location) = self.lower_current_source_location(source, substitution, locals, ty)
        {
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
            export::ExprKind::ConstructorParam(parameter) => concrete::ExprKind::ConstructorParam(
                concrete::ConstructorParamId::from_raw(parameter.into_raw()),
            ),
            export::ExprKind::VariantConstruct { variant, args } => {
                concrete::ExprKind::VariantConstruct {
                    variant: self.lower_applied_enum_variant_ref(*variant, substitution),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::VariantTest { operand, variant } => concrete::ExprKind::VariantTest {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                variant: self.lower_applied_enum_variant_ref(*variant, substitution),
            },
            export::ExprKind::VariantPayloadProject { operand, field } => {
                concrete::ExprKind::VariantPayloadProject {
                    operand: Box::new(self.lower_expr(operand, substitution, locals)),
                    field: self.lower_applied_enum_variant_field_ref(*field, substitution),
                }
            }
            export::ExprKind::Local(local) => {
                concrete::ExprKind::Local(self.lower_local(*local, locals))
            }
            export::ExprKind::GlobalRead(global) => {
                concrete::ExprKind::GlobalRead(self.global_map[global])
            }
            export::ExprKind::SingletonValue(value) => concrete::ExprKind::SingletonValue(
                concrete::SingletonValueId::from_raw(value.into_raw()),
            ),
            export::ExprKind::Capture(binding) => {
                concrete::ExprKind::Capture(concrete::BindingId::from_raw(binding.into_raw()))
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
            export::ExprKind::InitializingClassFieldAccess { application, field } => {
                let receiver_ty = self.lower_type(
                    self.source.class_applications[*application].canonical_type,
                    substitution,
                );
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(concrete::Expr {
                        kind: concrete::ExprKind::ConstructorReceiver,
                        ty: receiver_ty,
                        span: source.span,
                        origin: source.origin.concrete(),
                    }),
                    field: self.lower_field_ref(
                        export::FieldRef::ClassField {
                            application: *application,
                            field: *field,
                        },
                        substitution,
                    ),
                }
            }
            export::ExprKind::InitializingStructFieldAccess { application, index } => {
                let receiver_ty = self.lower_type(
                    self.source.struct_applications[*application].canonical_type,
                    substitution,
                );
                let structure = self.lower_struct_application(*application, substitution);
                let field = concrete::StructFieldRef::checked(&self.structs, structure, *index)
                    .expect("an initializing struct field remains in range");
                concrete::ExprKind::FieldAccess {
                    receiver: Box::new(concrete::Expr {
                        kind: concrete::ExprKind::ConstructorReceiver,
                        ty: receiver_ty,
                        span: source.span,
                        origin: source.origin.concrete(),
                    }),
                    field: concrete::FieldRef::StructField(field),
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
            export::ExprKind::IsInstance { operand, check_ty } => concrete::ExprKind::IsInstance {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                check_ty: self.lower_type(*check_ty, substitution),
            },
            export::ExprKind::Cast { operand, optional } => concrete::ExprKind::Cast {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                optional: *optional,
            },
            export::ExprKind::ArrayLiteral(elements) => concrete::ExprKind::ArrayLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::ArrayAssembly(assembly) => {
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
                    result_type: self.lower_class_application(assembly.result_type, substitution),
                })
            }
            export::ExprKind::Index {
                access,
                receiver,
                index,
            } => concrete::ExprKind::Index {
                access: *access,
                receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
            },
            export::ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => concrete::ExprKind::ArraySet {
                access: *access,
                receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
                value: Box::new(self.lower_expr(value, substitution, locals)),
            },
            export::ExprKind::ArrayLen(array) => {
                concrete::ExprKind::ArrayLen(Box::new(self.lower_expr(array, substitution, locals)))
            }
            export::ExprKind::ArrayClone(array) => concrete::ExprKind::ArrayClone(Box::new(
                self.lower_expr(array, substitution, locals),
            )),
            export::ExprKind::Call { callee, args } => concrete::ExprKind::Call {
                callee: self.lower_callable(*callee, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },
            export::ExprKind::ImportedCoreCall { callee, args } => {
                concrete::ExprKind::ImportedCoreCall {
                    callee: self.imported_core_callable_map[callee],
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => {
                let (callee, function_arguments) =
                    self.lower_callable_with_arguments(*callee, substitution);
                concrete::ExprKind::LocalFunctionCall {
                    local_function: self.ensure_local_function(
                        *local_function,
                        &function_arguments,
                        locals,
                    ),
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
                let operation = match *operation {
                    export::IntegerOperation::NoGc {
                        kind,
                        operation,
                        target,
                    } => concrete::IntegerOperation::NoGc {
                        kind,
                        operation,
                        target: concrete::NoGcCallableRef::map_from_export(target, |source| {
                            self.lower_integer_callable(kind, source)
                        }),
                    },
                    export::IntegerOperation::Managed {
                        kind,
                        operation,
                        target,
                    } => concrete::IntegerOperation::Managed {
                        kind,
                        operation,
                        target: concrete::ManagedCallableRef::map_from_export(target, |source| {
                            self.lower_integer_callable(kind, source)
                        }),
                    },
                };
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
                conversion: concrete::IntegerConversion {
                    source: conversion.source,
                    target_kind: conversion.target_kind,
                    target: concrete::NoGcCallableRef::map_from_export(
                        conversion.target,
                        |source| self.lower_integer_callable(conversion.source, source),
                    ),
                },
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
            } => concrete::ExprKind::Unwrap {
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
                trap_on_none: *trap_on_none,
            },
        };
        concrete::Expr {
            kind,
            ty,
            span: source.span,
            origin: source.origin.concrete(),
        }
    }

    pub(super) fn lower_integer_callable(
        &mut self,
        kind: export::IntegerKind,
        function: export::FunctionId,
    ) -> concrete::FunctionId {
        let owner = self.protocols.fundamental_types.integers.owner(kind);
        let application = self.source.structs[owner].self_application;
        let owner = self.lower_struct_application(application, &[]);
        self.request_method(
            function,
            concrete::MethodOwner::Struct(owner),
            MethodRequest::Plain,
        )
    }

    fn lower_current_source_location(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        _locals: &[concrete::LocalId],
        ty: concrete::TypeId,
    ) -> Option<concrete::Expr> {
        let export::ExprKind::Call { callee, args } = &source.kind else {
            return None;
        };
        if !args.is_empty()
            || self.source.callable_function(*callee) != self.protocols.source_location.current
        {
            return None;
        }

        let origin = source.origin.concrete();
        let evaluation = origin.evaluation;
        let file = &self.source.source_files[evaluation.file as usize];
        assert_eq!(
            file.provider, evaluation.provider,
            "evaluation origin provider must match its source file"
        );
        let (line, column) = source_line_column(&file.source, evaluation.span.start);
        let file_name = file.name.clone();
        let context = &self.source.source_contexts[evaluation.context];
        assert_eq!(
            context.source(),
            &file.identity,
            "evaluation context source must match its source file"
        );
        let (function_name, type_name) = self.source.source_context_names(evaluation.context);
        let location_application =
            self.source.structs[self.protocols.source_location.location].self_application;
        let location = self.lower_struct_application(location_application, substitution);
        let string_type = self.lower_type(self.source.string, substitution);
        let long_type = self.lower_integer_type(export::IntegerKind::SIGNED_64, substitution);
        assert_eq!(
            self.struct_type[&location], ty,
            "current_source_location return type must be SourceLocation"
        );
        let literal = |kind, ty| concrete::Expr {
            kind,
            ty,
            span: source.span,
            origin,
        };
        Some(concrete::Expr {
            kind: concrete::ExprKind::StructInit {
                struct_id: location,
                args: vec![
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: file_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                    literal(
                        concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(
                            line as u64,
                        )),
                        long_type,
                    ),
                    literal(
                        concrete::ExprKind::IntegerLiteral(export::HirIntegerConstant::Signed64(
                            column as u64,
                        )),
                        long_type,
                    ),
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: function_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                    literal(
                        concrete::ExprKind::StringLiteral {
                            value: type_name,
                            owner: export::StringConstantOwner::CurrentDefinition,
                        },
                        string_type,
                    ),
                ],
            },
            ty,
            span: source.span,
            origin,
        })
    }
}

fn source_line_column(source: &str, offset: u32) -> (i64, i64) {
    let mut line = 1_i64;
    let mut column = 1_i64;
    for (index, character) in source.char_indices() {
        if index as u32 >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
