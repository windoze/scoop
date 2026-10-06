use super::*;

impl Lowerer {
    pub(super) fn materialize_imported_default_expression(
        &mut self,
        expression: &hir::DefaultExpressionV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        let definition =
            self.imported_default_definition_origin(expression.definition_origin(), context)?;
        let span = definition.span;
        let source = expression.evaluation_origin();
        let location = context
            .owner
            .source_location(source.source(), source.context())
            .ok_or(ImportedDefinitionOriginError::MissingSource {
                context: source.context(),
            })?;
        let evaluation = self.import_dependency_evaluation_origin(source, location)?;
        let origin = hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
            definition,
            evaluation,
        });
        let ty = self
            .imported_default_type_with_bindings(expression.result_type(), context.bindings)
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;

        use hir::DefaultExpressionKindV1 as Kind;
        if matches!(
            expression.kind(),
            Kind::Index { .. } | Kind::ArraySet { .. }
        ) {
            self.prepare_array_bounds_exception_type()
                .map_err(|error| {
                    ImportedDefaultMaterializationError::Plan(
                        error.diagnostic("array bounds exception type"),
                    )
                })?;
        }
        let kind = match expression.kind() {
            Kind::ForeignCallbackRegister {
                registration,
                closure,
            } => self.materialize_imported_callback_registration(
                *registration,
                closure,
                definition,
                context,
            )?,
            Kind::ForeignCallbackOperation {
                operation,
                callback,
            } => hir::ExprKind::ForeignCallbackOperation {
                operation: (*operation).into(),
                callback: Box::new(
                    self.materialize_imported_default_expression(callback, context)?,
                ),
            },
            Kind::ContextLookup {
                declaration,
                parameter,
                diagnostic,
            } => self.materialize_context_lookup(
                *declaration,
                *parameter,
                diagnostic,
                expression.result_type(),
                context,
            )?,
            Kind::ReleaseFieldLoad {
                owner_type,
                declaration,
            } => hir::ExprKind::ReleaseFieldLoad(hir::ReleaseFieldRef {
                owner: self
                    .imported_default_type_with_bindings(owner_type, context.bindings)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?,
                field: *declaration,
            }),
            Kind::FunctionAddress(declaration) => {
                let target = hir::DefaultCallableRefV1::try_new(
                    *declaration,
                    scoop_identity::OptionalSignatureType::Absent,
                    Vec::new(),
                )
                .expect("a native function address has no receiver or type arguments");
                hir::ExprKind::FunctionAddress(self.materialize_imported_callable_target(
                    &target,
                    MemberCallKind::Ordinary,
                    context,
                )?)
            }
            Kind::GenericDelegateStorageRead(reference) => {
                hir::ExprKind::GenericDelegateStorageRead(
                    self.materialize_imported_delegate_reference(reference, context)?,
                )
            }
            Kind::Capture(index) => {
                let binding = context
                    .captures
                    .get(*index as usize)
                    .copied()
                    .ok_or_else(|| {
                        ImportedDefaultMaterializationError::Plan(format!(
                            "dependency body has no closure input {index}"
                        ))
                    })?;
                hir::ExprKind::Capture(binding)
            }
            kind @ (Kind::Lambda(_) | Kind::AnonymousFunction(_)) => {
                self.materialize_imported_closure(kind, ty, origin, context)?
            }
            Kind::CallableReference(reference) => {
                self.materialize_imported_callable_reference(reference, origin, context)?
            }
            Kind::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => {
                let source_type =
                    self.materialize_imported_function_type(source_function_type, context)?;
                let target_type =
                    self.materialize_imported_function_type(target_function_type, context)?;
                self.retain_boxing_sources(
                    self.function_types[source_type].canonical_type,
                    self.function_types[target_type].canonical_type,
                    span,
                );
                hir::ExprKind::FunctionCoercion {
                    source: Box::new(
                        self.materialize_imported_default_expression(source, context)?,
                    ),
                    coercion: self.function_coercion(source_type, target_type),
                    target_type,
                }
            }
            Kind::CallableCall {
                callee,
                function_type,
                arguments,
            } => {
                let function_type =
                    self.materialize_imported_function_type(function_type, context)?;
                hir::ExprKind::CallableCall {
                    callee: Box::new(
                        self.materialize_imported_default_expression(callee, context)?,
                    ),
                    function_type,
                    args: self.materialize_imported_default_expressions(arguments, context)?,
                }
            }
            kind @ (Kind::PtrFromNonZeroULong(_)
            | Kind::CharCode(_)
            | Kind::CharFromCodeUnchecked(_)
            | Kind::PtrToULong(_)
            | Kind::PtrCast(_)
            | Kind::PtrLoad { .. }
            | Kind::PtrStore { .. }
            | Kind::PtrOffset { .. }
            | Kind::AddressOf(_)
            | Kind::SizeOf(_)
            | Kind::AlignOf(_)) => self.materialize_imported_pointer_expression(kind, context)?,
            Kind::SingletonValue(value) => {
                if let Some(hir::SourceNominalId::GenericTemplate(owner)) =
                    self.imported_nominal_owner(ty)
                {
                    self.request_imported_companion(owner)
                        .map_err(ImportedDefaultMaterializationError::Plan)?;
                }
                hir::ExprKind::SingletonValue(hir::SingletonValueTarget::Dependency(*value))
            }
            Kind::SomeWrap(value) => hir::ExprKind::SomeWrap(Box::new(
                self.materialize_imported_default_expression(value, context)?,
            )),
            Kind::NoneLiteral => hir::ExprKind::NoneLiteral,
            Kind::IsSome(value) => hir::ExprKind::IsSome(Box::new(
                self.materialize_imported_default_expression(value, context)?,
            )),
            Kind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let trap_on_none = bool::from(*trap_on_none);
                if trap_on_none {
                    self.prepare_unwrap_exception_type().map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(format!(
                            "cannot resolve unwrap exception type: {error:?}"
                        ))
                    })?;
                }
                hir::ExprKind::Unwrap {
                    operand: Box::new(
                        self.materialize_imported_default_expression(operand, context)?,
                    ),
                    trap_on_none,
                }
            }
            Kind::ReferenceUpcast(operand) => hir::ExprKind::ReferenceUpcast(Box::new(
                self.materialize_imported_default_expression(operand, context)?,
            )),
            Kind::Box(operand) => {
                let operand = self.materialize_imported_default_expression(operand, context)?;
                self.retain_imported_box_source(operand.ty)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(
                            error.diagnostic("boxed value type"),
                        )
                    })?;
                hir::ExprKind::Box(Box::new(operand))
            }
            Kind::Unbox(operand) => hir::ExprKind::Unbox(Box::new(
                self.materialize_imported_default_expression(operand, context)?,
            )),
            Kind::IsInstance {
                operand,
                checked_type,
            } => hir::ExprKind::IsInstance {
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
                check_ty: self
                    .imported_default_type_with_bindings(checked_type, context.bindings)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?,
            },
            Kind::Cast {
                operand,
                checked_type,
                optional,
            } => {
                let optional = bool::from(*optional);
                if !optional {
                    self.prepare_cast_exception_type().map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(format!(
                            "cannot resolve cast exception type: {error:?}"
                        ))
                    })?;
                }
                hir::ExprKind::Cast {
                    operand: Box::new(
                        self.materialize_imported_default_expression(operand, context)?,
                    ),
                    check_ty: self
                        .imported_default_type_with_bindings(checked_type, context.bindings)
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(error.to_string())
                        })?,
                    optional,
                }
            }
            Kind::StringLiteral {
                value,
                owner: hir::DefaultStringOwnerV1::CurrentInstantiation,
            } => hir::ExprKind::StringLiteral {
                value: value.clone(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            Kind::IntegerLiteral(value) => hir::ExprKind::IntegerLiteral((*value).into()),
            Kind::FloatLiteral(value) => hir::ExprKind::FloatLiteral(*value),
            Kind::FloatUnary {
                kind,
                operation,
                operand,
            } => hir::ExprKind::FloatUnary {
                kind: *kind,
                operation: *operation,
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => hir::ExprKind::FloatBinary {
                kind: *kind,
                operation: *operation,
                lhs: Box::new(self.materialize_imported_default_expression(lhs, context)?),
                rhs: Box::new(self.materialize_imported_default_expression(rhs, context)?),
            },
            Kind::FloatConversion {
                conversion,
                operand,
            } => hir::ExprKind::FloatConversion {
                conversion: conversion.map_integer(hir::IntegerKind::from),
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::CharLiteral(value) => hir::ExprKind::CharLiteral((*value).into()),
            Kind::BooleanLiteral(value) => hir::ExprKind::BoolLiteral((*value).into()),
            Kind::UnitLiteral => hir::ExprKind::UnitLiteral,
            Kind::TupleLiteral(elements) => hir::ExprKind::TupleLiteral(
                self.materialize_imported_default_expressions(elements, context)?,
            ),
            Kind::ArrayLiteral(elements) => hir::ExprKind::ArrayLiteral(
                self.materialize_imported_default_expressions(elements, context)?,
            ),
            Kind::ArrayAssembly(assembly) => hir::ExprKind::ArrayAssembly(
                self.materialize_imported_array_assembly(assembly, ty, context)?,
            ),
            Kind::Index {
                access,
                receiver,
                index,
            } => hir::ExprKind::Index {
                access: (*access).into(),
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                index: Box::new(self.materialize_imported_default_expression(index, context)?),
            },
            Kind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => hir::ExprKind::ArraySet {
                access: (*access).into(),
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                index: Box::new(self.materialize_imported_default_expression(index, context)?),
                value: Box::new(self.materialize_imported_default_expression(value, context)?),
            },
            Kind::ArrayGenerate { count, initializer } => {
                self.prepare_array_size_exception_type().map_err(|error| {
                    ImportedDefaultMaterializationError::Plan(
                        error.diagnostic("array size exception type"),
                    )
                })?;
                hir::ExprKind::ArrayGenerate {
                    count: Box::new(self.materialize_imported_default_expression(count, context)?),
                    initializer: Box::new(
                        self.materialize_imported_default_expression(initializer, context)?,
                    ),
                }
            }
            Kind::ArrayLen(array) => hir::ExprKind::ArrayLen(Box::new(
                self.materialize_imported_default_expression(array, context)?,
            )),
            Kind::ArrayClone(array) => hir::ExprKind::ArrayClone(Box::new(
                self.materialize_imported_default_expression(array, context)?,
            )),
            Kind::StructConstruct { fields, .. } => {
                let hir::Type::Struct(application) = self.types[ty] else {
                    return Err(ImportedDefaultMaterializationError::Plan(
                        "a struct construction requires its complete struct application".into(),
                    ));
                };
                hir::ExprKind::StructConstruct {
                    application,
                    fields: self.materialize_imported_default_expressions(fields, context)?,
                }
            }
            Kind::VariantConstruct { variant, arguments } => {
                let owner = self
                    .imported_default_type_with_bindings(variant.owner_type(), context.bindings)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?;
                hir::ExprKind::VariantConstruct {
                    variant: hir::EnumVariantApplication {
                        owner,
                        variant: variant.declaration(),
                    },
                    args: self.materialize_imported_default_expressions(arguments, context)?,
                }
            }
            Kind::Local(local) => {
                let mut value = context.locals.get(local).cloned().ok_or_else(|| {
                    ImportedDefaultMaterializationError::UnknownLocal(local.clone())
                })?;
                value.ty = ty;
                value.span = span;
                value.origin = origin;
                return Ok(value);
            }
            Kind::Call {
                callee,
                arguments,
                receiver,
            } => {
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                let receiver = receiver
                    .as_ref()
                    .try_map(|ty| self.imported_default_type_with_bindings(ty, context.bindings))
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?;
                self.imported_template_call_kind(
                    callee,
                    args,
                    receiver,
                    MemberCallKind::Ordinary,
                    context,
                )?
            }
            Kind::LocalFunctionCall {
                callee,
                captures,
                arguments,
                ..
            } => {
                let mut args = self.materialize_imported_default_expressions(captures, context)?;
                args.extend(self.materialize_imported_default_expressions(arguments, context)?);
                self.imported_template_call_kind(
                    callee,
                    args,
                    hir::SourceCallReceiver::NoReceiver,
                    MemberCallKind::Ordinary,
                    context,
                )?
            }
            Kind::StructInit {
                constructor: constructor @ hir::DefaultConstructorRefV1::Struct { declaration, .. },
                arguments,
            }
            | Kind::ClassInit {
                constructor:
                    constructor @ hir::DefaultConstructorRefV1::Class {
                        declaration: hir::DefaultClassConstructorIdV1::Source(declaration),
                        ..
                    },
                arguments,
            } => {
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                let owner =
                    self.materialize_imported_default_type(constructor.owner_type(), context)?;
                if matches!(
                    self.imported_nominal_owner(owner),
                    Some(hir::SourceNominalId::GenericTemplate(_))
                ) {
                    let application =
                        self.imported_constructor_application(constructor, context.bindings)?;
                    match application {
                        hir::ConstructorApplicationRef::Class(constructor) => {
                            hir::ExprKind::ClassInit { constructor, args }
                        }
                        hir::ConstructorApplicationRef::Struct(constructor) => {
                            hir::ExprKind::StructInit { constructor, args }
                        }
                    }
                } else {
                    self.imported_default_call_kind(
                        scoop_identity::CallableTemplateOrigin::Constructor(*declaration),
                        args,
                        hir::SourceCallReceiver::NoReceiver,
                        MemberCallKind::Ordinary,
                    )?
                }
            }
            Kind::FieldAccess { receiver, field } => hir::ExprKind::FieldAccess {
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                field: self.materialize_imported_field_ref(field, context.bindings)?,
            },
            Kind::MethodCall {
                receiver,
                callee: hir::DefaultMethodCalleeV1::Callable(callee),
                arguments,
            }
            | Kind::DirectSuperMethodCall {
                receiver,
                callee: hir::DefaultMethodCalleeV1::Callable(callee),
                arguments,
            } => {
                let kind = if matches!(expression.kind(), Kind::DirectSuperMethodCall { .. }) {
                    MemberCallKind::DirectSuper
                } else {
                    MemberCallKind::Ordinary
                };
                let receiver =
                    Box::new(self.materialize_imported_default_expression(receiver, context)?);
                let callee = hir::MethodCallee::Callable(
                    self.materialize_imported_callable_target(callee, kind, context)?,
                );
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                match kind {
                    MemberCallKind::Ordinary => hir::ExprKind::MethodCall {
                        receiver,
                        callee,
                        args,
                    },
                    MemberCallKind::DirectSuper => hir::ExprKind::DirectSuperMethodCall {
                        receiver,
                        callee,
                        args,
                    },
                }
            }
            Kind::MethodCall {
                receiver,
                callee,
                arguments,
            } => hir::ExprKind::MethodCall {
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                callee: self.materialize_imported_method_callee(callee, origin, context)?,
                args: self.materialize_imported_default_expressions(arguments, context)?,
            },
            Kind::GlobalRead(property) => {
                self.materialize_imported_global_read(*property, ty, span, origin)?
            }
            kind @ (Kind::PrimitiveBinary { .. }
            | Kind::PrimitiveUnary { .. }
            | Kind::IntegerOperation { .. }
            | Kind::IntegerConversion { .. }
            | Kind::Binary { .. }
            | Kind::Unary { .. }) => self.materialize_imported_default_operator(kind, context)?,
            Kind::StringLiteral {
                owner: hir::DefaultStringOwnerV1::Property(_),
                ..
            }
            | Kind::StructInit { .. }
            | Kind::ClassInit { .. }
            | Kind::VariantTest { .. }
            | Kind::VariantPayloadProject { .. }
            | Kind::DirectSuperMethodCall { .. } => {
                return Err(ImportedDefaultMaterializationError::Plan(
                    "preflight admitted an unsupported dependency default operation".to_owned(),
                ));
            }
        };
        Ok(hir::Expr {
            kind,
            ty,
            span,
            origin,
        })
    }
}
