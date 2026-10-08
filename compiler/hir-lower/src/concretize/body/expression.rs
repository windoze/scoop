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
            .or_else(|| {
                self.current_source_site(self.concrete_expression_origin(source.origin).evaluation)
            })
            .or(previous);
        let expression = self.lower_expr_at_site(source, substitution, locals);
        if let concrete::ExprKind::Call {
            callee: concrete::CallableTarget::Imported(callee),
            ..
        } = expression.kind
            && self.imported_dependency_callables[callee].effect()
                == scoop_identity::Effect::Suspend
        {
            self.coroutine_results.insert(expression.ty);
        }
        if let Some(ty) = expression.shared_representation_type(&self.types) {
            self.shared_types.insert(ty);
        }
        self.type_use_site = previous;
        expression
    }

    fn concrete_expression_origin(
        &self,
        source: export::ExpressionOrigin,
    ) -> export::ConcreteExpressionOrigin {
        let mut origin = source.concrete();
        if let Some(context) = self.evaluation_context {
            origin.evaluation.context = context;
        }
        origin
    }

    fn lower_expr_at_site(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let ty = self.lower_type(source.ty, substitution);
        let origin = self.concrete_expression_origin(source.origin);
        if let Some(location) = self.lower_current_source_location(source, substitution, ty, origin)
        {
            return location;
        }
        let kind = match &source.kind {
            export::ExprKind::ContextLookup(requirement) => {
                self.lower_missing_context_exception_type();
                concrete::ExprKind::ContextLookup {
                    declaration: requirement.diagnostic.declaration.clone(),
                    label: requirement.diagnostic.label.clone(),
                    parameter: requirement.parameter,
                }
            }
            export::ExprKind::ReleaseFieldLoad(field) => {
                let concrete::FieldRef::ClassField { class_id, index } = self.lower_field_ref(
                    export::FieldRef::ClassField {
                        owner: field.owner,
                        field: field.field,
                    },
                    substitution,
                ) else {
                    unreachable!("a release field is a class backing field")
                };
                concrete::ExprKind::ReleaseFieldLoad {
                    class: class_id,
                    index,
                }
            }
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
            export::ExprKind::FloatLiteral(value) => concrete::ExprKind::FloatLiteral(*value),
            export::ExprKind::FloatUnary {
                kind,
                operation,
                operand,
            } => concrete::ExprKind::FloatUnary {
                kind: *kind,
                operation: *operation,
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
            },
            export::ExprKind::FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => concrete::ExprKind::FloatBinary {
                kind: *kind,
                operation: *operation,
                lhs: Box::new(self.lower_expr(lhs, substitution, locals)),
                rhs: Box::new(self.lower_expr(rhs, substitution, locals)),
            },
            export::ExprKind::FloatConversion {
                conversion,
                operand,
            } => concrete::ExprKind::FloatConversion {
                conversion: *conversion,
                operand: Box::new(self.lower_expr(operand, substitution, locals)),
            },
            export::ExprKind::CharLiteral(value) => concrete::ExprKind::CharLiteral(*value),
            export::ExprKind::CharCode(value) => {
                concrete::ExprKind::CharCode(Box::new(self.lower_expr(value, substitution, locals)))
            }
            export::ExprKind::CharFromCodeUnchecked(value) => {
                concrete::ExprKind::CharFromCodeUnchecked(Box::new(self.lower_expr(
                    value,
                    substitution,
                    locals,
                )))
            }
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
                concrete::ExprKind::SingletonValue(match *value {
                    export::SingletonValueTarget::Local(_) => {
                        concrete::SingletonValueTarget::Local(self.singleton_value_for_type(ty))
                    }
                    export::SingletonValueTarget::Dependency(value) => {
                        if let concrete::TypeKind::Class(class) = self.types[ty].kind
                            && self.object_type_map.contains_key(&class)
                        {
                            concrete::SingletonValueTarget::Local(self.singleton_value_for_type(ty))
                        } else {
                            concrete::SingletonValueTarget::Dependency(value)
                        }
                    }
                })
            }
            export::ExprKind::Capture(binding) => {
                concrete::ExprKind::Capture(concrete::BindingId::from_raw(binding.into_raw()))
            }
            export::ExprKind::Lambda(id) => {
                concrete::ExprKind::Lambda(self.lower_lambda(*id, substitution, locals))
            }
            export::ExprKind::AnonymousFunction(id) => concrete::ExprKind::AnonymousFunction(
                self.lower_anonymous(*id, substitution, locals),
            ),
            export::ExprKind::CallableReference(id) => concrete::ExprKind::CallableReference(
                self.lower_reference(*id, substitution, locals),
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
            export::ExprKind::AddressOf(place) => concrete::ExprKind::AddressOf(self.lower_place(
                place,
                locals,
                substitution,
                source.span,
            )),
            export::ExprKind::SizeOf(size) => {
                concrete::ExprKind::SizeOf(self.lower_type(*size, substitution))
            }
            export::ExprKind::AlignOf(align) => {
                concrete::ExprKind::AlignOf(self.lower_type(*align, substitution))
            }
            export::ExprKind::FunctionAddress(function) => concrete::ExprKind::FunctionAddress(
                self.lower_callable_target(*function, substitution),
            ),
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
            } => {
                self.lower_imported_callback_support();
                concrete::ExprKind::ForeignCallbackOperation {
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
                }
            }
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
                        origin,
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
                        origin,
                    }),
                    field,
                }
            }
            export::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            }
            | export::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                if self.is_nothing_type(receiver.ty) {
                    // Substitution may make a bound receiver uninhabited. Its
                    // evaluation ends before member selection or argument evaluation.
                    return receiver;
                }
                self.lower_method_call(
                    receiver,
                    *callee,
                    args,
                    matches!(source.kind, export::ExprKind::DirectSuperMethodCall { .. }),
                    substitution,
                    locals,
                )
            }
            export::ExprKind::Box(value) => {
                let value = self.lower_expr(value, substitution, locals);
                if self.is_nothing_type(value.ty) {
                    return value;
                }
                self.ensure_box_source(value.ty);
                if self.is_value_representation(value.ty) {
                    concrete::ExprKind::Box(Box::new(value))
                } else {
                    // A source type parameter with interface-only bounds is
                    // conservatively represented as Box in Export HIR. Its
                    // concrete argument may instead be a reference; in that
                    // case the adaptation is a zero-cost retype.
                    concrete::ExprKind::ReferenceUpcast(Box::new(value))
                }
            }
            export::ExprKind::Unbox(value) => {
                let value = self.lower_expr(value, substitution, locals);
                if self.is_value_representation(ty) {
                    concrete::ExprKind::Unbox(Box::new(value))
                } else {
                    concrete::ExprKind::ReferenceUpcast(Box::new(value))
                }
            }
            export::ExprKind::ReferenceUpcast(value) => {
                let value = self.lower_expr(value, substitution, locals);
                if self.is_nothing_type(value.ty) {
                    return value;
                }
                concrete::ExprKind::ReferenceUpcast(Box::new(value))
            }
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
            export::ExprKind::ArrayGenerate { count, initializer } => {
                self.lower_array_size_exception_type();
                concrete::ExprKind::ArrayGenerate {
                    count: Box::new(self.lower_expr(count, substitution, locals)),
                    initializer: Box::new(self.lower_expr(initializer, substitution, locals)),
                }
            }
            export::ExprKind::ArrayLen(array) => {
                concrete::ExprKind::ArrayLen(Box::new(self.lower_expr(array, substitution, locals)))
            }
            export::ExprKind::ArrayClone(array) => concrete::ExprKind::ArrayClone(Box::new(
                self.lower_expr(array, substitution, locals),
            )),
            export::ExprKind::AtomicNew(initial) => concrete::ExprKind::AtomicNew(Box::new(
                self.lower_expr(initial, substitution, locals),
            )),
            export::ExprKind::Atomic(atomic) => concrete::ExprKind::Atomic(Box::new(
                atomic.map(|value| self.lower_expr(value, substitution, locals)),
            )),
            export::ExprKind::Call {
                callee,
                binding,
                args,
                receiver,
            } => concrete::ExprKind::Call {
                binding: binding.clone(),
                receiver: receiver.map(|ty| self.lower_type(ty, substitution)),
                callee: self.lower_callable_target(*callee, substitution),
                args: args
                    .iter()
                    .map(|argument| self.lower_expr(argument, substitution, locals))
                    .collect(),
            },

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
            origin,
        }
    }
}
