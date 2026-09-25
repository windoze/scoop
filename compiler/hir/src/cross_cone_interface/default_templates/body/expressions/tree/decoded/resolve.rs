use std::fmt;

use scoop_identity::PersistentIdResolver;

use super::super::{
    DefaultArrayAssemblyBuildError, DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultIntegerArgumentsV1,
    OptionalDefaultExpressionV1,
};
use super::{
    DecodedDefaultArrayAssemblyPartV1, DecodedDefaultArrayAssemblyV1,
    DecodedDefaultExpressionKindV1, DecodedDefaultExpressionV1, DecodedDefaultIntegerArgumentsV1,
    DecodedOptionalDefaultExpressionV1, DefaultExpressionReferenceResolver,
    DefaultExpressionResolutionError,
};
use crate::TemplateLocalSelectorResolver;

impl DecodedDefaultExpressionV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultExpressionV1, DefaultExpressionResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let kind = self.kind.resolve(resolver, locals)?;
        let result_type = self.result_type.resolve(resolver).map_err(|error| {
            DefaultExpressionResolutionError::Type {
                variant_tag: 0,
                field: 2,
                error,
            }
        })?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(DefaultExpressionResolutionError::DefinitionOrigin)?;
        DefaultExpressionV1::try_new(kind, result_type, definition_origin)
            .map_err(DefaultExpressionResolutionError::Record)
    }
}

impl DecodedDefaultExpressionKindV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultExpressionKindV1, DefaultExpressionResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        Ok(match self {
            Self::StringLiteral { value, owner } => DefaultExpressionKindV1::StringLiteral {
                value,
                owner: owner
                    .resolve(resolver)
                    .map_err(DefaultExpressionResolutionError::StringOwner)?,
            },
            Self::IntegerLiteral(value) => DefaultExpressionKindV1::IntegerLiteral(value),
            Self::BooleanLiteral(value) => DefaultExpressionKindV1::BooleanLiteral(value),
            Self::UnitLiteral => DefaultExpressionKindV1::UnitLiteral,
            Self::TupleLiteral(elements) => DefaultExpressionKindV1::TupleLiteral(
                resolve_sequence(elements, resolver, locals, 5, 1)?,
            ),
            Self::StructInit {
                constructor,
                arguments,
            } => DefaultExpressionKindV1::StructInit {
                constructor: constructor.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Constructor {
                        variant_tag: 6,
                        error,
                    }
                })?,
                arguments: resolve_sequence(arguments, resolver, locals, 6, 2)?,
            },
            Self::StructConstruct { owner_type, fields } => {
                DefaultExpressionKindV1::StructConstruct {
                    owner_type: resolve_type(owner_type, resolver, 7, 1)?,
                    fields: resolve_sequence(fields, resolver, locals, 7, 2)?,
                }
            }
            Self::ClassInit {
                constructor,
                arguments,
            } => DefaultExpressionKindV1::ClassInit {
                constructor: constructor.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Constructor {
                        variant_tag: 8,
                        error,
                    }
                })?,
                arguments: resolve_sequence(arguments, resolver, locals, 8, 2)?,
            },
            Self::VariantConstruct { variant, arguments } => {
                DefaultExpressionKindV1::VariantConstruct {
                    variant: variant.resolve(resolver).map_err(|error| {
                        DefaultExpressionResolutionError::Variant {
                            variant_tag: 9,
                            error,
                        }
                    })?,
                    arguments: resolve_sequence(arguments, resolver, locals, 9, 2)?,
                }
            }
            Self::VariantTest { operand, variant } => DefaultExpressionKindV1::VariantTest {
                operand: resolve_child(operand, resolver, locals, 10, 1)?,
                variant: variant.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Variant {
                        variant_tag: 10,
                        error,
                    }
                })?,
            },
            Self::VariantPayloadProject { operand, field } => {
                DefaultExpressionKindV1::VariantPayloadProject {
                    operand: resolve_child(operand, resolver, locals, 11, 1)?,
                    field: field
                        .resolve(resolver)
                        .map_err(DefaultExpressionResolutionError::VariantField)?,
                }
            }
            Self::Local(local_index) => DefaultExpressionKindV1::Local(
                locals
                    .resolve_template_local_selector(local_index)
                    .map_err(DefaultExpressionResolutionError::Local)?,
            ),
            Self::GlobalRead(property) => {
                DefaultExpressionKindV1::GlobalRead(resolve_persistent(property, resolver, 13, 1)?)
            }
            Self::SingletonValue(value) => {
                DefaultExpressionKindV1::SingletonValue(resolve_persistent(value, resolver, 14, 1)?)
            }
            Self::Lambda(lambda) => DefaultExpressionKindV1::Lambda(
                lambda
                    .resolve(resolver, locals)
                    .map_err(DefaultExpressionResolutionError::Lambda)?,
            ),
            Self::AnonymousFunction(function) => DefaultExpressionKindV1::AnonymousFunction(
                function
                    .resolve(resolver, locals)
                    .map_err(DefaultExpressionResolutionError::AnonymousFunction)?,
            ),
            Self::CallableReference(reference) => DefaultExpressionKindV1::CallableReference(
                reference.resolve(resolver, locals).map_err(|error| {
                    DefaultExpressionResolutionError::CallableReference(Box::new(error))
                })?,
            ),
            Self::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => DefaultExpressionKindV1::FunctionCoercion {
                source: resolve_child(source, resolver, locals, 18, 1)?,
                source_function_type: resolve_type(source_function_type, resolver, 18, 2)?,
                target_function_type: resolve_type(target_function_type, resolver, 18, 3)?,
            },
            Self::PtrFromNonZeroULong(operand) => DefaultExpressionKindV1::PtrFromNonZeroULong(
                resolve_child(operand, resolver, locals, 19, 1)?,
            ),
            Self::PtrToULong(operand) => DefaultExpressionKindV1::PtrToULong(resolve_child(
                operand, resolver, locals, 20, 1,
            )?),
            Self::PtrCast(operand) => {
                DefaultExpressionKindV1::PtrCast(resolve_child(operand, resolver, locals, 21, 1)?)
            }
            Self::PtrLoad { pointer, offset } => DefaultExpressionKindV1::PtrLoad {
                pointer: resolve_child(pointer, resolver, locals, 22, 1)?,
                offset: resolve_optional(offset, resolver, locals, 22, 2)?,
            },
            Self::PtrStore {
                pointer,
                offset,
                value,
            } => DefaultExpressionKindV1::PtrStore {
                pointer: resolve_child(pointer, resolver, locals, 23, 1)?,
                offset: resolve_optional(offset, resolver, locals, 23, 2)?,
                value: resolve_child(value, resolver, locals, 23, 3)?,
            },
            Self::PtrOffset {
                pointer,
                offset,
                subtract,
            } => DefaultExpressionKindV1::PtrOffset {
                pointer: resolve_child(pointer, resolver, locals, 24, 1)?,
                offset: resolve_child(offset, resolver, locals, 24, 2)?,
                subtract,
            },
            Self::AddressOf(place) => DefaultExpressionKindV1::AddressOf(
                place
                    .resolve(resolver, locals)
                    .map_err(DefaultExpressionResolutionError::Place)?,
            ),
            Self::SizeOf(queried_type) => {
                DefaultExpressionKindV1::SizeOf(resolve_type(queried_type, resolver, 26, 1)?)
            }
            Self::AlignOf(queried_type) => {
                DefaultExpressionKindV1::AlignOf(resolve_type(queried_type, resolver, 27, 1)?)
            }
            Self::FunctionAddress(declaration) => DefaultExpressionKindV1::FunctionAddress(
                declaration
                    .resolve(resolver)
                    .map_err(DefaultExpressionResolutionError::CallableDeclaration)?,
            ),
            Self::ForeignCallbackRegister {
                registration,
                closure,
            } => DefaultExpressionKindV1::ForeignCallbackRegister {
                registration: resolve_persistent(registration, resolver, 29, 1)?,
                closure: resolve_child(closure, resolver, locals, 29, 2)?,
            },
            Self::ForeignCallbackOperation {
                operation,
                callback,
            } => DefaultExpressionKindV1::ForeignCallbackOperation {
                operation,
                callback: resolve_child(callback, resolver, locals, 30, 2)?,
            },
            Self::FieldAccess { receiver, field } => DefaultExpressionKindV1::FieldAccess {
                receiver: resolve_child(receiver, resolver, locals, 31, 1)?,
                field: field
                    .resolve(resolver)
                    .map_err(DefaultExpressionResolutionError::Field)?,
            },
            Self::MethodCall {
                receiver,
                callee,
                arguments,
            } => DefaultExpressionKindV1::MethodCall {
                receiver: resolve_child(receiver, resolver, locals, 32, 1)?,
                callee: callee.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Method {
                        variant_tag: 32,
                        error,
                    }
                })?,
                arguments: resolve_sequence(arguments, resolver, locals, 32, 3)?,
            },
            Self::DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver: resolve_child(receiver, resolver, locals, 33, 1)?,
                callee: callee.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Method {
                        variant_tag: 33,
                        error,
                    }
                })?,
                arguments: resolve_sequence(arguments, resolver, locals, 33, 3)?,
            },
            Self::Box(operand) => {
                DefaultExpressionKindV1::Box(resolve_child(operand, resolver, locals, 34, 1)?)
            }
            Self::Unbox(operand) => {
                DefaultExpressionKindV1::Unbox(resolve_child(operand, resolver, locals, 35, 1)?)
            }
            Self::IsInstance {
                operand,
                checked_type,
            } => DefaultExpressionKindV1::IsInstance {
                operand: resolve_child(operand, resolver, locals, 36, 1)?,
                checked_type: resolve_type(checked_type, resolver, 36, 2)?,
            },
            Self::Cast {
                operand,
                checked_type,
                optional,
            } => DefaultExpressionKindV1::Cast {
                operand: resolve_child(operand, resolver, locals, 37, 1)?,
                checked_type: resolve_type(checked_type, resolver, 37, 2)?,
                optional,
            },
            Self::ArrayLiteral(elements) => DefaultExpressionKindV1::ArrayLiteral(
                resolve_sequence(elements, resolver, locals, 38, 1)?,
            ),
            Self::ArrayAssembly(assembly) => {
                DefaultExpressionKindV1::ArrayAssembly(assembly.resolve(resolver, locals)?)
            }
            Self::Index {
                access,
                receiver,
                index,
            } => DefaultExpressionKindV1::Index {
                access,
                receiver: resolve_child(receiver, resolver, locals, 40, 2)?,
                index: resolve_child(index, resolver, locals, 40, 3)?,
            },
            Self::ArraySet {
                access,
                receiver,
                index,
                value,
            } => DefaultExpressionKindV1::ArraySet {
                access,
                receiver: resolve_child(receiver, resolver, locals, 41, 2)?,
                index: resolve_child(index, resolver, locals, 41, 3)?,
                value: resolve_child(value, resolver, locals, 41, 4)?,
            },
            Self::ArrayLen(operand) => {
                DefaultExpressionKindV1::ArrayLen(resolve_child(operand, resolver, locals, 42, 1)?)
            }
            Self::ArrayClone(operand) => DefaultExpressionKindV1::ArrayClone(resolve_child(
                operand, resolver, locals, 43, 1,
            )?),
            Self::Call { callee, arguments } => DefaultExpressionKindV1::Call {
                callee: resolve_callable(callee, resolver, 44, 1)?,
                arguments: resolve_sequence(arguments, resolver, locals, 44, 2)?,
            },
            Self::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => DefaultExpressionKindV1::LocalFunctionCall {
                declaration: declaration.resolve(resolver).map_err(|error| {
                    DefaultExpressionResolutionError::Persistent {
                        variant_tag: 45,
                        field: 1,
                        error,
                    }
                })?,
                callee: resolve_callable(callee, resolver, 45, 2)?,
                captures: resolve_sequence(captures, resolver, locals, 45, 3)?,
                arguments: resolve_sequence(arguments, resolver, locals, 45, 4)?,
            },
            Self::CallableCall {
                callee,
                function_type,
                arguments,
            } => DefaultExpressionKindV1::CallableCall {
                callee: resolve_child(callee, resolver, locals, 46, 1)?,
                function_type: resolve_type(function_type, resolver, 46, 2)?,
                arguments: resolve_sequence(arguments, resolver, locals, 46, 3)?,
            },
            Self::PrimitiveBinary { kind, lhs, rhs } => DefaultExpressionKindV1::PrimitiveBinary {
                kind,
                lhs: resolve_child(lhs, resolver, locals, 47, 2)?,
                rhs: resolve_child(rhs, resolver, locals, 47, 3)?,
            },
            Self::PrimitiveUnary { kind, operand } => DefaultExpressionKindV1::PrimitiveUnary {
                kind,
                operand: resolve_child(operand, resolver, locals, 48, 2)?,
            },
            Self::IntegerOperation {
                operation,
                arguments,
            } => DefaultExpressionKindV1::IntegerOperation {
                operation,
                arguments: arguments.resolve(resolver, locals)?,
            },
            Self::IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => DefaultExpressionKindV1::IntegerConversion {
                source_kind,
                target_kind,
                operand: resolve_child(operand, resolver, locals, 50, 3)?,
            },
            Self::Binary { operator, lhs, rhs } => DefaultExpressionKindV1::Binary {
                operator,
                lhs: resolve_child(lhs, resolver, locals, 51, 2)?,
                rhs: resolve_child(rhs, resolver, locals, 51, 3)?,
            },
            Self::Unary { operator, operand } => DefaultExpressionKindV1::Unary {
                operator,
                operand: resolve_child(operand, resolver, locals, 52, 2)?,
            },
            Self::SomeWrap(operand) => {
                DefaultExpressionKindV1::SomeWrap(resolve_child(operand, resolver, locals, 53, 1)?)
            }
            Self::NoneLiteral => DefaultExpressionKindV1::NoneLiteral,
            Self::IsSome(operand) => {
                DefaultExpressionKindV1::IsSome(resolve_child(operand, resolver, locals, 55, 1)?)
            }
            Self::Unwrap {
                operand,
                trap_on_none,
            } => DefaultExpressionKindV1::Unwrap {
                operand: resolve_child(operand, resolver, locals, 56, 1)?,
                trap_on_none,
            },
        })
    }
}

impl DecodedDefaultArrayAssemblyV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultArrayAssemblyV1, DefaultExpressionResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let element_type = resolve_type(self.element_type, resolver, 39, 1)?;
        let count = u32::try_from(self.parts.len()).map_err(|_| {
            DefaultExpressionResolutionError::ArrayAssembly(
                DefaultArrayAssemblyBuildError::TooManyParts,
            )
        })?;
        let mut parts = Vec::with_capacity(count as usize);
        for (index, part) in self.parts.into_iter().enumerate() {
            let part = match part {
                DecodedDefaultArrayAssemblyPartV1::Element(expression) => {
                    DefaultArrayAssemblyPartV1::Element(resolve_sequence_element(
                        expression, resolver, locals, 39, 2, index,
                    )?)
                }
                DecodedDefaultArrayAssemblyPartV1::CopyArray(expression) => {
                    DefaultArrayAssemblyPartV1::CopyArray(resolve_sequence_element(
                        expression, resolver, locals, 39, 2, index,
                    )?)
                }
            };
            parts.push(part);
        }
        let result_type = resolve_type(self.result_type, resolver, 39, 3)?;
        DefaultArrayAssemblyV1::try_new(element_type, parts, result_type)
            .map_err(DefaultExpressionResolutionError::ArrayAssembly)
    }
}

impl DecodedDefaultIntegerArgumentsV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultIntegerArgumentsV1, DefaultExpressionResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Unary(operand) => resolve_child(operand, resolver, locals, 49, 2)
                .map(DefaultIntegerArgumentsV1::Unary),
            Self::Binary { lhs, rhs } => Ok(DefaultIntegerArgumentsV1::Binary {
                lhs: resolve_indexed_child(lhs, resolver, locals, 49, 2, 0)?,
                rhs: resolve_indexed_child(rhs, resolver, locals, 49, 2, 1)?,
            }),
        }
    }
}

fn resolve_optional<R, L, E>(
    expression: DecodedOptionalDefaultExpressionV1,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<OptionalDefaultExpressionV1, DefaultExpressionResolutionError<E, L::Error>>
where
    R: DefaultExpressionReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    match expression {
        DecodedOptionalDefaultExpressionV1::Absent => Ok(OptionalDefaultExpressionV1::Absent),
        DecodedOptionalDefaultExpressionV1::Present(expression) => {
            resolve_child(expression, resolver, locals, variant_tag, field)
                .map(OptionalDefaultExpressionV1::Present)
        }
    }
}

fn resolve_child<R, L, E>(
    expression: Box<DecodedDefaultExpressionV1>,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<Box<DefaultExpressionV1>, DefaultExpressionResolutionError<E, L::Error>>
where
    R: DefaultExpressionReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    expression
        .resolve(resolver, locals)
        .map(Box::new)
        .map_err(|error| DefaultExpressionResolutionError::Nested {
            variant_tag,
            field,
            index: None,
            error: Box::new(error),
        })
}

fn resolve_indexed_child<R, L, E>(
    expression: Box<DecodedDefaultExpressionV1>,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
    index: usize,
) -> Result<Box<DefaultExpressionV1>, DefaultExpressionResolutionError<E, L::Error>>
where
    R: DefaultExpressionReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    resolve_sequence_element(*expression, resolver, locals, variant_tag, field, index).map(Box::new)
}

fn resolve_sequence<R, L, E>(
    expressions: Vec<DecodedDefaultExpressionV1>,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
) -> Result<Vec<DefaultExpressionV1>, DefaultExpressionResolutionError<E, L::Error>>
where
    R: DefaultExpressionReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    let mut resolved = Vec::with_capacity(expressions.len());
    for (index, expression) in expressions.into_iter().enumerate() {
        resolved.push(resolve_sequence_element(
            expression,
            resolver,
            locals,
            variant_tag,
            field,
            index,
        )?);
    }
    Ok(resolved)
}

fn resolve_sequence_element<R, L, E>(
    expression: DecodedDefaultExpressionV1,
    resolver: &mut R,
    locals: &mut L,
    variant_tag: u64,
    field: u32,
    index: usize,
) -> Result<DefaultExpressionV1, DefaultExpressionResolutionError<E, L::Error>>
where
    R: DefaultExpressionReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    expression
        .resolve(resolver, locals)
        .map_err(|error| DefaultExpressionResolutionError::Nested {
            variant_tag,
            field,
            index: Some(index),
            error: Box::new(error),
        })
}

fn resolve_type<R, L, E>(
    value: scoop_identity::DecodedSignatureTypeKey,
    resolver: &mut R,
    variant_tag: u64,
    field: u32,
) -> Result<scoop_identity::SignatureTypeKey, DefaultExpressionResolutionError<E, L>>
where
    R: DefaultExpressionReferenceResolver<E>,
{
    value
        .resolve(resolver)
        .map_err(|error| DefaultExpressionResolutionError::Type {
            variant_tag,
            field,
            error,
        })
}

fn resolve_persistent<R, L, E, I>(
    value: scoop_identity::DecodedPersistentId<I>,
    resolver: &mut R,
    variant_tag: u64,
    field: u32,
) -> Result<I, DefaultExpressionResolutionError<E, L>>
where
    I: scoop_identity::PersistentId,
    R: DefaultExpressionReferenceResolver<E> + PersistentIdResolver<I, Error = E>,
{
    resolver
        .resolve(value)
        .map_err(|error| DefaultExpressionResolutionError::Persistent {
            variant_tag,
            field,
            error,
        })
}

fn resolve_callable<R, L, E>(
    callable: crate::DecodedDefaultCallableRefV1,
    resolver: &mut R,
    variant_tag: u64,
    field: u32,
) -> Result<crate::DefaultCallableRefV1, DefaultExpressionResolutionError<E, L>>
where
    R: DefaultExpressionReferenceResolver<E>,
{
    callable
        .resolve(resolver)
        .map_err(|error| DefaultExpressionResolutionError::Callable {
            variant_tag,
            field,
            error,
        })
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display for DefaultExpressionResolutionError<E, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "invalid default expression local: {error}"),
            Self::Persistent {
                variant_tag,
                field,
                error,
            } => write!(
                formatter,
                "invalid default expression tag {variant_tag} field {field} identity: {error}"
            ),
            Self::Type {
                variant_tag,
                field,
                error,
            } => write!(
                formatter,
                "invalid default expression tag {variant_tag} field {field} type: {error}"
            ),
            Self::StringOwner(error) => {
                write!(formatter, "invalid default string literal: {error}")
            }
            Self::Constructor { variant_tag, error } => {
                write!(
                    formatter,
                    "invalid default expression tag {variant_tag} constructor: {error}"
                )
            }
            Self::Variant { variant_tag, error } => {
                write!(
                    formatter,
                    "invalid default expression tag {variant_tag} variant: {error}"
                )
            }
            Self::VariantField(error) => {
                write!(formatter, "invalid default variant payload field: {error}")
            }
            Self::Lambda(error) => write!(formatter, "invalid default lambda: {error}"),
            Self::AnonymousFunction(error) => {
                write!(formatter, "invalid default anonymous function: {error}")
            }
            Self::CallableReference(error) => {
                write!(formatter, "invalid default callable reference: {error}")
            }
            Self::Place(error) => write!(formatter, "invalid default address place: {error}"),
            Self::CallableDeclaration(error) => {
                write!(formatter, "invalid default function address: {error}")
            }
            Self::Field(error) => write!(formatter, "invalid default field access: {error}"),
            Self::Method { variant_tag, error } => {
                write!(
                    formatter,
                    "invalid default expression tag {variant_tag} method: {error}"
                )
            }
            Self::Callable {
                variant_tag,
                field,
                error,
            } => write!(
                formatter,
                "invalid default expression tag {variant_tag} field {field} callable: {error}"
            ),
            Self::DefinitionOrigin(error) => {
                write!(
                    formatter,
                    "invalid default expression definition origin: {error}"
                )
            }
            Self::Nested {
                variant_tag,
                field,
                index: Some(index),
                error,
            } => write!(
                formatter,
                "invalid default expression tag {variant_tag} field {field} element {index}: {error}"
            ),
            Self::Nested {
                variant_tag,
                field,
                index: None,
                error,
            } => write!(
                formatter,
                "invalid default expression tag {variant_tag} field {field}: {error}"
            ),
            Self::Record(error) => error.fmt(formatter),
            Self::ArrayAssembly(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultExpressionResolutionError<E, L>
{
}
