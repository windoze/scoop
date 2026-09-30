//! Expression-tree projection for portable defaults.

mod imported;
mod support;

use crate::{
    CanonicalBooleanV1, DefaultExpressionKindV1, DefaultExpressionV1, DefaultIntegerArgumentsV1,
    DefaultStringOwnerV1, Expr, ExprKind, HirIntegerOperationArguments, StringConstantOwner,
};

use super::BodyProjection;

impl BodyProjection<'_, '_> {
    pub(super) fn expression(
        &mut self,
        expression: &Expr,
    ) -> Result<DefaultExpressionV1, super::super::DefaultBodyProjectionError> {
        let kind = self.expression_kind(
            &expression.kind,
            expression.ty,
            expression.origin.definition(),
        )?;
        DefaultExpressionV1::try_new(
            kind,
            self.type_key(expression.ty)?,
            self.origin(expression.origin.definition())?,
            crate::production::definition_sources::project_evaluation_origin(
                self.entities.export(),
                expression.origin.concrete().evaluation,
            )?,
        )
        .map_err(super::super::DefaultBodyProjectionError::Expression)
    }

    fn expression_kind(
        &mut self,
        kind: &ExprKind,
        result_type: crate::TypeId,
        origin: crate::DefinitionOrigin,
    ) -> Result<DefaultExpressionKindV1, super::super::DefaultBodyProjectionError> {
        Ok(match kind {
            ExprKind::GenericDelegateStorageRead(reference) => {
                DefaultExpressionKindV1::GenericDelegateStorageRead(
                    self.generic_delegate_reference(reference)?,
                )
            }
            ExprKind::StringLiteral { value, owner } => DefaultExpressionKindV1::StringLiteral {
                value: value.clone(),
                owner: match owner {
                    StringConstantOwner::CurrentDefinition => {
                        DefaultStringOwnerV1::CurrentInstantiation
                    }
                    StringConstantOwner::Property(property) => {
                        DefaultStringOwnerV1::Property(self.entities.property_id(*property)?)
                    }
                },
            },
            ExprKind::IntegerLiteral(value) => {
                DefaultExpressionKindV1::IntegerLiteral((*value).into())
            }
            ExprKind::BoolLiteral(value) => {
                DefaultExpressionKindV1::BooleanLiteral((*value).into())
            }
            ExprKind::UnitLiteral => DefaultExpressionKindV1::UnitLiteral,
            ExprKind::TupleLiteral(elements) => {
                DefaultExpressionKindV1::TupleLiteral(self.expressions(elements)?)
            }
            ExprKind::StructInit { constructor, args } => DefaultExpressionKindV1::StructInit {
                constructor: self
                    .entities
                    .struct_constructor(*constructor, self.binders)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::StructConstruct {
                application,
                fields,
            } => DefaultExpressionKindV1::StructConstruct {
                owner_type: self.struct_application_type(*application)?,
                fields: self.expressions(fields)?,
            },
            ExprKind::ClassInit { constructor, args } => DefaultExpressionKindV1::ClassInit {
                constructor: self
                    .entities
                    .class_constructor(*constructor, self.binders)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::ConstructorReceiver => {
                DefaultExpressionKindV1::Local(scoop_identity::LocalValueSelector::This)
            }
            ExprKind::ImportedConstructorInit { application, args } => {
                let constructor = self
                    .entities
                    .imported_constructor_application(*application, self.binders)?;
                let arguments = self.expressions(args)?;
                match constructor {
                    crate::DefaultConstructorRefV1::Struct { .. } => {
                        DefaultExpressionKindV1::StructInit {
                            constructor,
                            arguments,
                        }
                    }
                    crate::DefaultConstructorRefV1::Class { .. } => {
                        DefaultExpressionKindV1::ClassInit {
                            constructor,
                            arguments,
                        }
                    }
                    crate::DefaultConstructorRefV1::Variant { .. } => {
                        unreachable!("class/struct construction cannot select a variant")
                    }
                }
            }
            ExprKind::ConstructorParam(parameter) => {
                DefaultExpressionKindV1::Local(self.locals.constructor_parameter(*parameter)?)
            }
            ExprKind::VariantConstruct { variant, args } => {
                DefaultExpressionKindV1::VariantConstruct {
                    variant: self.entities.variant(*variant, self.binders)?,
                    arguments: self.expressions(args)?,
                }
            }
            ExprKind::VariantTest { operand, variant } => DefaultExpressionKindV1::VariantTest {
                operand: Box::new(self.expression(operand)?),
                variant: self.entities.variant(*variant, self.binders)?,
            },
            ExprKind::VariantPayloadProject { operand, field } => {
                DefaultExpressionKindV1::VariantPayloadProject {
                    operand: Box::new(self.expression(operand)?),
                    field: self.entities.variant_field(*field, self.binders)?,
                }
            }
            ExprKind::Local(local) => DefaultExpressionKindV1::Local(self.local(*local)?),
            ExprKind::GlobalRead(global) => {
                DefaultExpressionKindV1::GlobalRead(self.entities.global_property(*global)?)
            }
            ExprKind::SingletonValue(value) => {
                DefaultExpressionKindV1::SingletonValue(self.entities.singleton_id(*value)?)
            }
            ExprKind::ImportedSingletonValue(value) => {
                DefaultExpressionKindV1::SingletonValue(*value)
            }
            ExprKind::Capture(binding) => match self.locals.capture_source(*binding)? {
                crate::DefaultCaptureSourceV1::Local(selector) => {
                    DefaultExpressionKindV1::Local(selector)
                }
                crate::DefaultCaptureSourceV1::EnclosingCapture(index) => {
                    DefaultExpressionKindV1::Capture(index)
                }
            },
            ExprKind::Lambda(lambda) => DefaultExpressionKindV1::Lambda(self.lambda(*lambda)?),
            ExprKind::ImportedClosure(closure) => self.imported_closure(closure)?,
            ExprKind::ImportedCallableReference(reference) => {
                DefaultExpressionKindV1::CallableReference(
                    self.imported_callable_reference(reference)?,
                )
            }
            ExprKind::AnonymousFunction(function) => {
                DefaultExpressionKindV1::AnonymousFunction(self.anonymous_function(*function)?)
            }
            ExprKind::CallableReference(reference) => {
                DefaultExpressionKindV1::CallableReference(self.callable_reference(*reference)?)
            }
            ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let coercion =
                    super::super::arena_get(&self.entities.export().function_coercions, *coercion)
                        .ok_or_else(|| self.unknown("function coercion", *coercion))?;
                DefaultExpressionKindV1::FunctionCoercion {
                    source: Box::new(self.expression(source)?),
                    source_function_type: self.function_type(coercion.source)?,
                    target_function_type: self.function_type(*target_type)?,
                }
            }
            ExprKind::PtrFromNonZeroULong(value) => {
                DefaultExpressionKindV1::PtrFromNonZeroULong(Box::new(self.expression(value)?))
            }
            ExprKind::PtrToULong(value) => {
                DefaultExpressionKindV1::PtrToULong(Box::new(self.expression(value)?))
            }
            ExprKind::PtrCast(value) => {
                DefaultExpressionKindV1::PtrCast(Box::new(self.expression(value)?))
            }
            ExprKind::PtrLoad { pointer, offset } => DefaultExpressionKindV1::PtrLoad {
                pointer: Box::new(self.expression(pointer)?),
                offset: self.optional_expression(offset.as_deref())?,
            },
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => DefaultExpressionKindV1::PtrStore {
                pointer: Box::new(self.expression(pointer)?),
                offset: self.optional_expression(offset.as_deref())?,
                value: Box::new(self.expression(value)?),
            },
            ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => DefaultExpressionKindV1::PtrOffset {
                pointer: Box::new(self.expression(pointer)?),
                offset: Box::new(self.expression(offset)?),
                subtract: CanonicalBooleanV1::from(*subtract),
            },
            ExprKind::AddressOf(place) => DefaultExpressionKindV1::AddressOf(self.place(*place)?),
            ExprKind::SizeOf(ty) => DefaultExpressionKindV1::SizeOf(self.type_key(*ty)?),
            ExprKind::AlignOf(ty) => DefaultExpressionKindV1::AlignOf(self.type_key(*ty)?),
            ExprKind::FunctionAddress(function) => DefaultExpressionKindV1::FunctionAddress(
                self.entities.callable_declaration(*function)?,
            ),
            ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => DefaultExpressionKindV1::ForeignCallbackRegister {
                registration: self.entities.callback_id(*registration)?,
                closure: Box::new(self.expression(closure)?),
            },
            ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => DefaultExpressionKindV1::ForeignCallbackOperation {
                operation: (*operation).into(),
                callback: Box::new(self.expression(callback)?),
            },
            ExprKind::FieldAccess { receiver, field } => DefaultExpressionKindV1::FieldAccess {
                receiver: Box::new(self.expression(receiver)?),
                field: self.entities.field(*field, self.binders)?,
            },
            ExprKind::InitializingClassFieldAccess { field } => {
                let (receiver, field) = self.initializing_class_field(*field, origin)?;
                DefaultExpressionKindV1::FieldAccess {
                    receiver: Box::new(receiver),
                    field,
                }
            }
            ExprKind::InitializingStructFieldAccess { owner, field } => {
                let ty = self.type_key(*owner)?;
                DefaultExpressionKindV1::FieldAccess {
                    receiver: Box::new(self.initializing_receiver(ty, origin)?),
                    field: self.entities.field(
                        crate::FieldRef::StructField {
                            owner: *owner,
                            field: *field,
                        },
                        self.binders,
                    )?,
                }
            }
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => DefaultExpressionKindV1::MethodCall {
                receiver: Box::new(self.expression(receiver)?),
                callee: self.method_callee(*callee)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver: Box::new(self.expression(receiver)?),
                callee: self.method_callee(*callee)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::Box(value) => DefaultExpressionKindV1::Box(Box::new(self.expression(value)?)),
            ExprKind::Unbox(value) => {
                DefaultExpressionKindV1::Unbox(Box::new(self.expression(value)?))
            }
            ExprKind::ReferenceUpcast(value) => {
                DefaultExpressionKindV1::ReferenceUpcast(Box::new(self.expression(value)?))
            }
            ExprKind::IsInstance { operand, check_ty } => DefaultExpressionKindV1::IsInstance {
                operand: Box::new(self.expression(operand)?),
                checked_type: self.type_key(*check_ty)?,
            },
            ExprKind::Cast {
                operand,
                check_ty,
                optional,
            } => DefaultExpressionKindV1::Cast {
                operand: Box::new(self.expression(operand)?),
                checked_type: self.type_key(*check_ty)?,
                optional: CanonicalBooleanV1::from(*optional),
            },
            ExprKind::ArrayLiteral(elements) => {
                DefaultExpressionKindV1::ArrayLiteral(self.expressions(elements)?)
            }
            ExprKind::ArrayAssembly(assembly) => {
                DefaultExpressionKindV1::ArrayAssembly(self.array_assembly(assembly)?)
            }
            ExprKind::Index {
                access,
                receiver,
                index,
            } => DefaultExpressionKindV1::Index {
                access: (*access).into(),
                receiver: Box::new(self.expression(receiver)?),
                index: Box::new(self.expression(index)?),
            },
            ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => DefaultExpressionKindV1::ArraySet {
                access: (*access).into(),
                receiver: Box::new(self.expression(receiver)?),
                index: Box::new(self.expression(index)?),
                value: Box::new(self.expression(value)?),
            },
            ExprKind::ArrayLen(value) => {
                DefaultExpressionKindV1::ArrayLen(Box::new(self.expression(value)?))
            }
            ExprKind::ArrayClone(value) => {
                DefaultExpressionKindV1::ArrayClone(Box::new(self.expression(value)?))
            }
            ExprKind::Call {
                callee,
                args,
                receiver,
                ..
            } => match callee {
                crate::CallableTarget::Local(_) => DefaultExpressionKindV1::Call {
                    callee: self.callable_target(*callee)?,
                    receiver: receiver.try_map(|ty| self.type_key(ty))?,
                    arguments: self.expressions(args)?,
                },
                crate::CallableTarget::Application(application) => self.imported_generic_call(
                    *application,
                    args,
                    *receiver,
                    crate::MemberCallKind::Ordinary,
                )?,
                crate::CallableTarget::Dependency(callee) => {
                    self.imported_call(*callee, args, *receiver, result_type)?
                }
            },
            ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => DefaultExpressionKindV1::LocalFunctionCall {
                declaration: self.local_function_declaration(*local_function)?,
                callee: self.entities.callable(*callee, self.binders)?,
                captures: self.expressions(captures)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => DefaultExpressionKindV1::CallableCall {
                callee: Box::new(self.expression(callee)?),
                function_type: self.function_type(*function_type)?,
                arguments: self.expressions(args)?,
            },
            ExprKind::PrimitiveBinary { kind, lhs, rhs } => {
                DefaultExpressionKindV1::PrimitiveBinary {
                    kind: (*kind).into(),
                    lhs: Box::new(self.expression(lhs)?),
                    rhs: Box::new(self.expression(rhs)?),
                }
            }
            ExprKind::PrimitiveUnary { kind, operand } => DefaultExpressionKindV1::PrimitiveUnary {
                kind: (*kind).into(),
                operand: Box::new(self.expression(operand)?),
            },
            ExprKind::IntegerOperation {
                operation,
                arguments,
            } => DefaultExpressionKindV1::IntegerOperation {
                operation: (*operation).into(),
                arguments: match arguments {
                    HirIntegerOperationArguments::Unary(operand) => {
                        DefaultIntegerArgumentsV1::unary(self.expression(operand)?)
                    }
                    HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        DefaultIntegerArgumentsV1::binary(
                            self.expression(lhs)?,
                            self.expression(rhs)?,
                        )
                    }
                },
            },
            ExprKind::IntegerConversion {
                conversion,
                operand,
            } => DefaultExpressionKindV1::IntegerConversion {
                source_kind: conversion.source.into(),
                target_kind: conversion.target_kind.into(),
                operand: Box::new(self.expression(operand)?),
            },
            ExprKind::Binary { op, lhs, rhs } => DefaultExpressionKindV1::Binary {
                operator: (*op).into(),
                lhs: Box::new(self.expression(lhs)?),
                rhs: Box::new(self.expression(rhs)?),
            },
            ExprKind::Unary { op, operand } => DefaultExpressionKindV1::Unary {
                operator: (*op).into(),
                operand: Box::new(self.expression(operand)?),
            },
            ExprKind::SomeWrap(value) => {
                DefaultExpressionKindV1::SomeWrap(Box::new(self.expression(value)?))
            }
            ExprKind::NoneLiteral => DefaultExpressionKindV1::NoneLiteral,
            ExprKind::IsSome(value) => {
                DefaultExpressionKindV1::IsSome(Box::new(self.expression(value)?))
            }
            ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => DefaultExpressionKindV1::Unwrap {
                operand: Box::new(self.expression(operand)?),
                trap_on_none: CanonicalBooleanV1::from(*trap_on_none),
            },
        })
    }
}
