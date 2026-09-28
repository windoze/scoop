use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_expr(
        &mut self,
        source: &hir::Expr,
        context: &mut InstantiationContext,
    ) -> hir::Expr {
        if let hir::ExprKind::Local(local) = source.kind {
            let mut value = context.locals[arena_index(local)].clone();
            value.span = source.span;
            value.origin = instantiate_origin(source.origin, context.evaluation);
            return value;
        }
        if let hir::ExprKind::Capture(binding) = source.kind
            && let Some(value) = context.captures.get(&binding)
        {
            let mut value = value.clone();
            value.span = source.span;
            value.origin = instantiate_origin(source.origin, context.evaluation);
            return value;
        }
        let kind = match &source.kind {
            hir::ExprKind::StringLiteral { value, owner } => hir::ExprKind::StringLiteral {
                value: value.clone(),
                owner: *owner,
            },
            hir::ExprKind::IntegerLiteral(value) => hir::ExprKind::IntegerLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => hir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => hir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => hir::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_expr(element, context))
                    .collect(),
            ),
            hir::ExprKind::StructInit { constructor, args } => hir::ExprKind::StructInit {
                constructor: self.instantiate_default_struct_constructor(*constructor, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::StructConstruct {
                application,
                fields,
            } => hir::ExprKind::StructConstruct {
                application: self.instantiate_default_struct_application(*application, context),
                fields: self.instantiate_default_exprs(fields, context),
            },
            hir::ExprKind::ClassInit { constructor, args } => hir::ExprKind::ClassInit {
                constructor: self.instantiate_default_class_constructor(*constructor, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ConstructorReceiver => hir::ExprKind::ConstructorReceiver,
            hir::ExprKind::ImportedConstructorInit { application, args } => {
                let application = self.imported_constructor_applications[*application].clone();
                let owner = self.instantiate_method_ty(application.owner, &context.bindings);
                let application = self.imported_constructor_applications.alloc(
                    hir::ImportedConstructorApplication {
                        template: application.template,
                        owner,
                    },
                );
                hir::ExprKind::ImportedConstructorInit {
                    application,
                    args: self.instantiate_default_exprs(args, context),
                }
            }
            hir::ExprKind::ConstructorParam(parameter) => {
                hir::ExprKind::ConstructorParam(*parameter)
            }
            hir::ExprKind::VariantConstruct { variant, args } => hir::ExprKind::VariantConstruct {
                variant: self.instantiate_default_applied_enum_variant(*variant, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ImportedVariantConstruct {
                owner,
                variant,
                args,
            } => hir::ExprKind::ImportedVariantConstruct {
                owner: self.instantiate_method_ty(*owner, &context.bindings),
                variant: *variant,
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::VariantTest { operand, variant } => hir::ExprKind::VariantTest {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                variant: self.instantiate_default_applied_enum_variant(*variant, context),
            },
            hir::ExprKind::VariantPayloadProject { operand, field } => {
                hir::ExprKind::VariantPayloadProject {
                    operand: Box::new(self.instantiate_default_expr(operand, context)),
                    field: self.instantiate_default_applied_enum_field(*field, context),
                }
            }
            hir::ExprKind::Local(_) => unreachable!("local reads return before kind cloning"),
            hir::ExprKind::GlobalRead(global) => hir::ExprKind::GlobalRead(*global),
            hir::ExprKind::SingletonValue(value) => hir::ExprKind::SingletonValue(*value),
            hir::ExprKind::ImportedSingletonValue(value) => {
                hir::ExprKind::ImportedSingletonValue(*value)
            }
            hir::ExprKind::Capture(binding) => hir::ExprKind::Capture(*binding),
            hir::ExprKind::Lambda(lambda) => {
                hir::ExprKind::Lambda(self.instantiate_default_lambda(*lambda, context))
            }
            hir::ExprKind::AnonymousFunction(function) => hir::ExprKind::AnonymousFunction(
                self.instantiate_default_anonymous(*function, context),
            ),
            hir::ExprKind::CallableReference(reference) => hir::ExprKind::CallableReference(
                self.instantiate_default_reference(*reference, context),
            ),
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => hir::ExprKind::FunctionCoercion {
                source: Box::new(self.instantiate_default_expr(source, context)),
                coercion: self.instantiate_default_coercion(*coercion, context),
                target_type: self.instantiate_default_function_type(*target_type, context),
            },
            hir::ExprKind::PtrFromNonZeroULong(value) => hir::ExprKind::PtrFromNonZeroULong(
                Box::new(self.instantiate_default_expr(value, context)),
            ),
            hir::ExprKind::PtrToULong(value) => {
                hir::ExprKind::PtrToULong(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::PtrCast(value) => {
                hir::ExprKind::PtrCast(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::PtrLoad { pointer, offset } => hir::ExprKind::PtrLoad {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: offset
                    .as_ref()
                    .map(|value| Box::new(self.instantiate_default_expr(value, context))),
            },
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => hir::ExprKind::PtrStore {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: offset
                    .as_ref()
                    .map(|value| Box::new(self.instantiate_default_expr(value, context))),
                value: Box::new(self.instantiate_default_expr(value, context)),
            },
            hir::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => hir::ExprKind::PtrOffset {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: Box::new(self.instantiate_default_expr(offset, context)),
                subtract: *subtract,
            },
            hir::ExprKind::AddressOf(place) => hir::ExprKind::AddressOf(match place {
                hir::Place::Local(local) => hir::Place::Local(mapped_local(context, *local)),
                hir::Place::Global(global) => hir::Place::Global(*global),
            }),
            hir::ExprKind::SizeOf(ty) => {
                hir::ExprKind::SizeOf(self.instantiate_method_ty(*ty, &context.bindings))
            }
            hir::ExprKind::AlignOf(ty) => {
                hir::ExprKind::AlignOf(self.instantiate_method_ty(*ty, &context.bindings))
            }
            hir::ExprKind::FunctionAddress(function) => hir::ExprKind::FunctionAddress(*function),
            hir::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => hir::ExprKind::ForeignCallbackRegister {
                registration: self.instantiate_default_foreign_callback(*registration, context),
                closure: Box::new(self.instantiate_default_expr(closure, context)),
            },
            hir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => hir::ExprKind::ForeignCallbackOperation {
                operation: *operation,
                callback: Box::new(self.instantiate_default_expr(callback, context)),
            },
            hir::ExprKind::FieldAccess { receiver, field } => hir::ExprKind::FieldAccess {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                field: self.instantiate_default_field(*field, context),
            },
            hir::ExprKind::InitializingClassFieldAccess { field } => {
                hir::ExprKind::InitializingClassFieldAccess {
                    field: self.instantiate_default_initializing_field(*field, context),
                }
            }
            hir::ExprKind::InitializingStructFieldAccess { application, index } => {
                hir::ExprKind::InitializingStructFieldAccess {
                    application: self.instantiate_default_struct_application(*application, context),
                    index: *index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => hir::ExprKind::MethodCall {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                callee: self.instantiate_default_method_callee(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => hir::ExprKind::DirectSuperMethodCall {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                callee: self.instantiate_default_method_callee(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::Box(value) => {
                hir::ExprKind::Box(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Unbox(value) => {
                hir::ExprKind::Unbox(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::ReferenceUpcast(value) => hir::ExprKind::ReferenceUpcast(Box::new(
                self.instantiate_default_expr(value, context),
            )),
            hir::ExprKind::IsInstance { operand, check_ty } => hir::ExprKind::IsInstance {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                check_ty: self.instantiate_method_ty(*check_ty, &context.bindings),
            },
            hir::ExprKind::Cast {
                operand,
                check_ty,
                optional,
            } => hir::ExprKind::Cast {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                check_ty: self.instantiate_method_ty(*check_ty, &context.bindings),
                optional: *optional,
            },
            hir::ExprKind::ArrayLiteral(elements) => {
                hir::ExprKind::ArrayLiteral(self.instantiate_default_exprs(elements, context))
            }
            hir::ExprKind::ArrayAssembly(assembly) => {
                hir::ExprKind::ArrayAssembly(hir::ArrayAssembly {
                    element_type: self
                        .instantiate_method_ty(assembly.element_type, &context.bindings),
                    parts: assembly
                        .parts
                        .iter()
                        .map(|part| match part {
                            hir::ArrayAssemblyPart::Element(value) => {
                                hir::ArrayAssemblyPart::Element(
                                    self.instantiate_default_expr(value, context),
                                )
                            }
                            hir::ArrayAssemblyPart::CopyArray(value) => {
                                hir::ArrayAssemblyPart::CopyArray(
                                    self.instantiate_default_expr(value, context),
                                )
                            }
                        })
                        .collect(),
                    result_type: self
                        .instantiate_default_class_application(assembly.result_type, context),
                })
            }
            hir::ExprKind::Index {
                access,
                receiver,
                index,
            } => hir::ExprKind::Index {
                access: *access,
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
            },
            hir::ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => hir::ExprKind::ArraySet {
                access: *access,
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
                value: Box::new(self.instantiate_default_expr(value, context)),
            },
            hir::ExprKind::ArrayLen(value) => {
                hir::ExprKind::ArrayLen(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::ArrayClone(value) => {
                hir::ExprKind::ArrayClone(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Call {
                callee,
                args,
                receiver,
            } => hir::ExprKind::Call {
                receiver: receiver.map(|ty| self.instantiate_method_ty(ty, &context.bindings)),
                callee: self.instantiate_default_callable(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },

            hir::ExprKind::ImportedDependencyCall {
                callee,
                binding,
                args,
                receiver,
            } => hir::ExprKind::ImportedDependencyCall {
                receiver: receiver.map(|ty| self.instantiate_method_ty(ty, &context.bindings)),
                callee: *callee,
                binding: binding.clone(),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ImportedGenericCall {
                application,
                binding,
                args,
                receiver,
            } => {
                let application = self.imported_generic_applications[*application].clone();
                let arguments = application
                    .arguments
                    .iter()
                    .copied()
                    .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
                    .collect::<Vec<_>>();
                let application = self.imported_generic_applications.alloc(
                    hir::ImportedGenericCallableApplication {
                        template: application.template,
                        arguments: hir::NonEmptyVec::from_vec(arguments)
                            .expect("generic application retains its arguments"),
                    },
                );
                hir::ExprKind::ImportedGenericCall {
                    application,
                    binding: binding.clone(),
                    args: self.instantiate_default_exprs(args, context),
                    receiver: receiver.map(|ty| self.instantiate_method_ty(ty, &context.bindings)),
                }
            }
            hir::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => hir::ExprKind::LocalFunctionCall {
                local_function: context.local_function(*local_function),
                callee: self.instantiate_default_callable(*callee, context),
                captures: self.instantiate_default_exprs(captures, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => hir::ExprKind::CallableCall {
                callee: Box::new(self.instantiate_default_expr(callee, context)),
                function_type: self.instantiate_default_function_type(*function_type, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::PrimitiveBinary { kind, lhs, rhs } => hir::ExprKind::PrimitiveBinary {
                kind: *kind,
                lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                rhs: Box::new(self.instantiate_default_expr(rhs, context)),
            },
            hir::ExprKind::PrimitiveUnary { kind, operand } => hir::ExprKind::PrimitiveUnary {
                kind: *kind,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::IntegerOperation {
                operation,
                arguments,
            } => hir::ExprKind::IntegerOperation {
                operation: *operation,
                arguments: match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => {
                        hir::HirIntegerOperationArguments::Unary(Box::new(
                            self.instantiate_default_expr(operand, context),
                        ))
                    }
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        hir::HirIntegerOperationArguments::Binary {
                            lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                            rhs: Box::new(self.instantiate_default_expr(rhs, context)),
                        }
                    }
                },
            },
            hir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => hir::ExprKind::IntegerConversion {
                conversion: *conversion,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::Binary { op, lhs, rhs } => hir::ExprKind::Binary {
                op: *op,
                lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                rhs: Box::new(self.instantiate_default_expr(rhs, context)),
            },
            hir::ExprKind::Unary { op, operand } => hir::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::SomeWrap(value) => {
                hir::ExprKind::SomeWrap(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::NoneLiteral => hir::ExprKind::NoneLiteral,
            hir::ExprKind::IsSome(value) => {
                hir::ExprKind::IsSome(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => hir::ExprKind::Unwrap {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                trap_on_none: *trap_on_none,
            },
        };
        hir::Expr {
            kind,
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            span: source.span,
            origin: instantiate_origin(source.origin, context.evaluation),
        }
    }

    pub(super) fn instantiate_default_exprs(
        &mut self,
        source: &[hir::Expr],
        context: &mut InstantiationContext,
    ) -> Vec<hir::Expr> {
        source
            .iter()
            .map(|value| self.instantiate_default_expr(value, context))
            .collect()
    }
}
