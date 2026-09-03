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
            export::ExprKind::StringLiteral(value) => {
                concrete::ExprKind::StringLiteral(value.clone())
            }
            export::ExprKind::IntLiteral(value) => concrete::ExprKind::IntLiteral(*value),
            export::ExprKind::BoolLiteral(value) => concrete::ExprKind::BoolLiteral(*value),
            export::ExprKind::UnitLiteral => concrete::ExprKind::UnitLiteral,
            export::ExprKind::TupleLiteral(elements) => concrete::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, substitution, locals))
                    .collect(),
            ),
            export::ExprKind::StructInit { application, args } => {
                let id = self.lower_struct_application(*application, substitution);
                concrete::ExprKind::StructInit {
                    struct_id: id,
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ClassInit { application, args } => {
                let concrete_id = self.lower_class_application(*application, substitution);
                concrete::ExprKind::ClassInit {
                    constructor: self.class_constructor_by_class[&concrete_id],
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::ConstructorParam(parameter) => concrete::ExprKind::ConstructorParam(
                concrete::ConstructorParamId::from_raw(parameter.into_raw()),
            ),
            export::ExprKind::VariantConstruct {
                application,
                variant,
                args,
            } => {
                let id = self.lower_enum_application(*application, substitution);
                concrete::ExprKind::VariantConstruct {
                    enum_id: id,
                    variant: concrete::VariantId::from_raw(*variant),
                    args: args
                        .iter()
                        .map(|argument| self.lower_expr(argument, substitution, locals))
                        .collect(),
                }
            }
            export::ExprKind::Local(local) => {
                concrete::ExprKind::Local(self.lower_local(*local, locals))
            }
            export::ExprKind::GlobalRead(global) => {
                concrete::ExprKind::GlobalRead(self.global_map[global])
            }
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
            export::ExprKind::PtrFromUInt(value) => concrete::ExprKind::PtrFromUInt(Box::new(
                self.lower_expr(value, substitution, locals),
            )),
            export::ExprKind::PtrToUInt(value) => concrete::ExprKind::PtrToUInt(Box::new(
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
            export::ExprKind::FunPtrNull => concrete::ExprKind::FunPtrNull,
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
            export::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
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
            export::ExprKind::Box(value) => {
                concrete::ExprKind::Box(Box::new(self.lower_expr(value, substitution, locals)))
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
            || self.source.callable_function(*callee) != self.source.source_location_core.current
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
        let function_name = context.function_name.clone();
        let type_name = context.type_name.clone();
        let location_application =
            self.source.structs[self.source.source_location_core.location].self_application;
        let location = self.lower_struct_application(location_application, substitution);
        let string_type = self.lower_type(self.source.string, substitution);
        let int_type = self.lower_type(self.source.int, substitution);
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
                    literal(concrete::ExprKind::StringLiteral(file_name), string_type),
                    literal(concrete::ExprKind::IntLiteral(line), int_type),
                    literal(concrete::ExprKind::IntLiteral(column), int_type),
                    literal(
                        concrete::ExprKind::StringLiteral(function_name),
                        string_type,
                    ),
                    literal(concrete::ExprKind::StringLiteral(type_name), string_type),
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
