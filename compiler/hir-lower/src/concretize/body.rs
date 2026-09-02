use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_body(
        &mut self,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) -> (concrete::Body, Vec<concrete::LocalId>) {
        let mut locals = Arena::new();
        let mut local_map = Vec::with_capacity(source.locals.len());
        for (source_id, source_local) in source.locals.iter() {
            let id = locals.alloc(concrete::Local {
                binding: concrete::BindingId::from_raw(source_local.binding.into_raw()),
                name: source_local.name.clone(),
                ty: self.lower_type(source_local.ty, substitution),
                mutable: source_local.mutable,
            });
            assert_eq!(id.into_raw(), source_id.into_raw());
            local_map.push(id);
        }
        let statements = source
            .statements
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, &local_map))
            .collect();
        (concrete::Body { locals, statements }, local_map)
    }

    pub(super) fn lower_statement(
        &mut self,
        source: &export::Statement,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Option<concrete::Statement> {
        let kind = match &source.kind {
            export::StatementKind::Expr(expr) => {
                concrete::StatementKind::Expr(self.lower_expr(expr, substitution, locals))
            }
            // This marker has no runtime semantics. Concrete local-function
            // entities are requested by direct calls/references instead.
            export::StatementKind::LocalFunction(_) => return None,
            export::StatementKind::Return { value } => concrete::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.lower_expr(value, substitution, locals)),
            },
            export::StatementKind::ValDecl { pattern, init } => {
                let init = self.lower_expr(init, substitution, locals);
                let pattern = self.lower_pattern(pattern, init.ty, substitution, locals);
                concrete::StatementKind::ValDecl { pattern, init }
            }
            export::StatementKind::Assign { target, value } => concrete::StatementKind::Assign {
                target: self.lower_assign_target(target, substitution, locals),
                value: self.lower_expr(value, substitution, locals),
            },
            export::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => concrete::StatementKind::If {
                cond: self.lower_expr(cond, substitution, locals),
                then_body: self.lower_statements(then_body, substitution, locals),
                else_body: else_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            },
            export::StatementKind::While { cond, body } => concrete::StatementKind::While {
                cond: self.lower_expr(cond, substitution, locals),
                body: self.lower_statements(body, substitution, locals),
            },
            export::StatementKind::When(when) => {
                concrete::StatementKind::When(self.lower_when(when, substitution, locals))
            }
            export::StatementKind::Try(try_) => concrete::StatementKind::Try(concrete::Try {
                body: self.lower_statements(&try_.body, substitution, locals),
                catches: try_
                    .catches
                    .iter()
                    .map(|catch| concrete::CatchClause {
                        local: self.lower_local(catch.local, locals),
                        ty: self.lower_type(catch.ty, substitution),
                        body: self.lower_statements(&catch.body, substitution, locals),
                        span: catch.span,
                    })
                    .collect(),
                finally_body: try_
                    .finally_body
                    .as_ref()
                    .map(|body| self.lower_statements(body, substitution, locals)),
            }),
            export::StatementKind::Throw(expr) => {
                concrete::StatementKind::Throw(self.lower_expr(expr, substitution, locals))
            }
        };
        Some(concrete::Statement {
            kind,
            span: source.span,
        })
    }

    pub(super) fn lower_statements(
        &mut self,
        source: &[export::Statement],
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> Vec<concrete::Statement> {
        source
            .iter()
            .filter_map(|statement| self.lower_statement(statement, substitution, locals))
            .collect()
    }

    pub(super) fn lower_assign_target(
        &mut self,
        source: &export::AssignTarget,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::AssignTarget {
        match source {
            export::AssignTarget::Local(local) => {
                concrete::AssignTarget::Local(self.lower_local(*local, locals))
            }
            export::AssignTarget::Global(global) => {
                concrete::AssignTarget::Global(self.global_map[global])
            }
            export::AssignTarget::Index { array, index } => concrete::AssignTarget::Index {
                array: self.lower_expr(array, substitution, locals),
                index: self.lower_expr(index, substitution, locals),
            },
            export::AssignTarget::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver, substitution, locals);
                let field = self.lower_field_ref(*field, substitution);
                concrete::AssignTarget::Field {
                    receiver: Box::new(receiver),
                    field,
                }
            }
        }
    }

    pub(super) fn lower_when(
        &mut self,
        source: &export::When,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::When {
        let subject = self.lower_expr(&source.subject, substitution, locals);
        let arms = source
            .arms
            .iter()
            .map(|arm| concrete::WhenArm {
                pattern: self.lower_pattern(&arm.pattern, subject.ty, substitution, locals),
                guard: arm
                    .guard
                    .as_ref()
                    .map(|guard| self.lower_expr(guard, substitution, locals)),
                body: self.lower_statements(&arm.body, substitution, locals),
                span: arm.span,
            })
            .collect();
        concrete::When {
            subject,
            arms,
            else_body: source
                .else_body
                .as_ref()
                .map(|body| self.lower_statements(body, substitution, locals)),
        }
    }

    pub(super) fn lower_pattern(
        &mut self,
        source: &export::Pattern,
        expected: concrete::TypeId,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Pattern {
        match source {
            export::Pattern::Binding { local } => concrete::Pattern::Binding {
                local: self.lower_local(*local, locals),
            },
            export::Pattern::Wildcard => concrete::Pattern::Wildcard,
            export::Pattern::Literal {
                value,
                equals,
                subject_ty,
            } => concrete::Pattern::Literal {
                value: self.lower_expr(value, substitution, locals),
                equals: self.lower_callable(*equals, substitution),
                subject_ty: self.lower_type(*subject_ty, substitution),
            },
            export::Pattern::Variant {
                application,
                variant,
                fields,
            } => {
                let concrete_enum = self.lower_enum_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Enum(concrete_enum),
                    "the checked pattern application must match its subject"
                );
                let variant_id = concrete::VariantId::from_raw(*variant);
                let definition = self.enums[concrete_enum].variants[*variant as usize].clone();
                let fields = fields
                    .iter()
                    .map(|(field, pattern)| {
                        (
                            *field,
                            self.lower_pattern(
                                pattern,
                                definition.fields[*field as usize].ty,
                                substitution,
                                locals,
                            ),
                        )
                    })
                    .collect();
                concrete::Pattern::Variant {
                    enum_id: concrete_enum,
                    variant: variant_id,
                    fields,
                }
            }
            export::Pattern::Tuple(patterns) => {
                let concrete::TypeKind::Tuple(elements) = self.types[expected].kind.clone() else {
                    panic!("a resolved tuple pattern has a concrete tuple subject")
                };
                concrete::Pattern::Tuple(
                    patterns
                        .iter()
                        .zip(elements)
                        .map(|(pattern, expected)| {
                            self.lower_pattern(pattern, expected, substitution, locals)
                        })
                        .collect(),
                )
            }
            export::Pattern::Struct {
                application,
                fields,
            } => {
                let concrete_struct = self.lower_struct_application(*application, substitution);
                assert_eq!(
                    self.types[expected].kind,
                    concrete::TypeKind::Struct(concrete_struct),
                    "the checked pattern application must match its subject"
                );
                let definition = self.structs[concrete_struct].declared_fields().to_vec();
                concrete::Pattern::Struct {
                    struct_id: concrete_struct,
                    fields: fields
                        .iter()
                        .map(|(field, pattern)| {
                            (
                                *field,
                                self.lower_pattern(
                                    pattern,
                                    definition[*field as usize].ty,
                                    substitution,
                                    locals,
                                ),
                            )
                        })
                        .collect(),
                }
            }
        }
    }

    pub(super) fn lower_expr(
        &mut self,
        source: &export::Expr,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
    ) -> concrete::Expr {
        let ty = self.lower_type(source.ty, substitution);
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
            export::ExprKind::Index { receiver, index } => concrete::ExprKind::Index {
                receiver: Box::new(self.lower_expr(receiver, substitution, locals)),
                index: Box::new(self.lower_expr(index, substitution, locals)),
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
        }
    }
}
