use std::fmt;

use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

use super::{
    DefaultArrayAssemblyPartV1, DefaultArrayAssemblyV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultIntegerArgumentsV1, OptionalDefaultExpressionV1,
};
use crate::{
    CanonicalBooleanV1, CanonicalIntegerConstantV1, DefaultArrayAccessKindV1,
    DefaultBinaryOperatorV1, DefaultCallableDeclarationV1, DefaultCallableRefV1,
    DefaultConstructorRefV1, DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1,
    DefaultFieldRefV1, DefaultForeignCallbackOperationV1, DefaultIntegerKindV1,
    DefaultIntegerOperationV1, DefaultLexicalCallableIndexError, DefaultMethodCalleeV1,
    DefaultPlaceIndexError, DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1,
    DefaultStringOwnerV1, DefaultUnaryOperatorV1, IndexedDefaultAnonymousFunctionV1,
    IndexedDefaultLambdaV1, IndexedDefaultPlaceV1, TemplateLocalIndexResolver,
};

use super::super::{DefaultCallableReferenceIndexError, IndexedDefaultCallableReferenceV1};

#[derive(Debug)]
pub struct IndexedDefaultExpressionV1<'a> {
    expression: &'a DefaultExpressionV1,
    kind: IndexedDefaultExpressionKindV1<'a>,
}

#[derive(Debug)]
enum IndexedDefaultExpressionKindV1<'a> {
    StringLiteral {
        value: &'a str,
        owner: &'a DefaultStringOwnerV1,
    },
    IntegerLiteral(&'a CanonicalIntegerConstantV1),
    BooleanLiteral(CanonicalBooleanV1),
    UnitLiteral,
    TupleLiteral(Vec<IndexedDefaultExpressionV1<'a>>),
    StructInit {
        constructor: &'a DefaultConstructorRefV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    StructConstruct {
        owner_type: &'a SignatureTypeKey,
        fields: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    ClassInit {
        constructor: &'a DefaultConstructorRefV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    VariantConstruct {
        variant: &'a DefaultEnumVariantRefV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    VariantTest {
        operand: Box<IndexedDefaultExpressionV1<'a>>,
        variant: &'a DefaultEnumVariantRefV1,
    },
    VariantPayloadProject {
        operand: Box<IndexedDefaultExpressionV1<'a>>,
        field: &'a DefaultEnumVariantFieldRefV1,
    },
    Local(u32),
    Capture(u32),
    GlobalRead(&'a scoop_identity::PersistentPropertyId),
    GenericDelegateStorageRead(&'a crate::DefaultGenericDelegateReferenceV1),
    SingletonValue(&'a scoop_identity::PersistentObjectValueId),
    Lambda(IndexedDefaultLambdaV1<'a>),
    AnonymousFunction(IndexedDefaultAnonymousFunctionV1<'a>),
    CallableReference(IndexedDefaultCallableReferenceV1<'a>),
    FunctionCoercion {
        source: Box<IndexedDefaultExpressionV1<'a>>,
        source_function_type: &'a SignatureTypeKey,
        target_function_type: &'a SignatureTypeKey,
    },
    PtrFromNonZeroULong(Box<IndexedDefaultExpressionV1<'a>>),
    PtrToULong(Box<IndexedDefaultExpressionV1<'a>>),
    PtrCast(Box<IndexedDefaultExpressionV1<'a>>),
    PtrLoad {
        pointer: Box<IndexedDefaultExpressionV1<'a>>,
        offset: IndexedOptionalDefaultExpressionV1<'a>,
    },
    PtrStore {
        pointer: Box<IndexedDefaultExpressionV1<'a>>,
        offset: IndexedOptionalDefaultExpressionV1<'a>,
        value: Box<IndexedDefaultExpressionV1<'a>>,
    },
    PtrOffset {
        pointer: Box<IndexedDefaultExpressionV1<'a>>,
        offset: Box<IndexedDefaultExpressionV1<'a>>,
        subtract: CanonicalBooleanV1,
    },
    AddressOf(IndexedDefaultPlaceV1),
    SizeOf(&'a SignatureTypeKey),
    AlignOf(&'a SignatureTypeKey),
    FunctionAddress(&'a DefaultCallableDeclarationV1),
    ForeignCallbackRegister {
        registration: &'a scoop_identity::PersistentCallbackRegistrationId,
        closure: Box<IndexedDefaultExpressionV1<'a>>,
    },
    ForeignCallbackOperation {
        operation: DefaultForeignCallbackOperationV1,
        callback: Box<IndexedDefaultExpressionV1<'a>>,
    },
    ReleaseFieldLoad {
        owner_type: &'a scoop_identity::SignatureTypeKey,
        declaration: &'a scoop_identity::PersistentFieldId,
    },
    FieldAccess {
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        field: &'a DefaultFieldRefV1,
    },
    MethodCall {
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        callee: &'a DefaultMethodCalleeV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    DirectSuperMethodCall {
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        callee: &'a DefaultMethodCalleeV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    Box(Box<IndexedDefaultExpressionV1<'a>>),
    Unbox(Box<IndexedDefaultExpressionV1<'a>>),
    ReferenceUpcast(Box<IndexedDefaultExpressionV1<'a>>),
    IsInstance {
        operand: Box<IndexedDefaultExpressionV1<'a>>,
        checked_type: &'a SignatureTypeKey,
    },
    Cast {
        operand: Box<IndexedDefaultExpressionV1<'a>>,
        checked_type: &'a SignatureTypeKey,
        optional: CanonicalBooleanV1,
    },
    ArrayLiteral(Vec<IndexedDefaultExpressionV1<'a>>),
    ArrayGenerate {
        count: Box<IndexedDefaultExpressionV1<'a>>,
        initializer: Box<IndexedDefaultExpressionV1<'a>>,
    },
    ArrayAssembly(IndexedDefaultArrayAssemblyV1<'a>),
    Index {
        access: DefaultArrayAccessKindV1,
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        index: Box<IndexedDefaultExpressionV1<'a>>,
    },
    ArraySet {
        access: DefaultArrayAccessKindV1,
        receiver: Box<IndexedDefaultExpressionV1<'a>>,
        index: Box<IndexedDefaultExpressionV1<'a>>,
        value: Box<IndexedDefaultExpressionV1<'a>>,
    },
    ArrayLen(Box<IndexedDefaultExpressionV1<'a>>),
    ArrayClone(Box<IndexedDefaultExpressionV1<'a>>),
    Call {
        callee: &'a DefaultCallableRefV1,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
        receiver: &'a crate::SourceCallReceiver<SignatureTypeKey>,
    },
    LocalFunctionCall {
        declaration: &'a CallableTemplateOrigin,
        callee: &'a DefaultCallableRefV1,
        captures: Vec<IndexedDefaultExpressionV1<'a>>,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    CallableCall {
        callee: Box<IndexedDefaultExpressionV1<'a>>,
        function_type: &'a SignatureTypeKey,
        arguments: Vec<IndexedDefaultExpressionV1<'a>>,
    },
    PrimitiveBinary {
        kind: DefaultPrimitiveBinaryKindV1,
        lhs: Box<IndexedDefaultExpressionV1<'a>>,
        rhs: Box<IndexedDefaultExpressionV1<'a>>,
    },
    PrimitiveUnary {
        kind: DefaultPrimitiveUnaryKindV1,
        operand: Box<IndexedDefaultExpressionV1<'a>>,
    },
    IntegerOperation {
        operation: &'a DefaultIntegerOperationV1,
        arguments: IndexedDefaultIntegerArgumentsV1<'a>,
    },
    IntegerConversion {
        source_kind: DefaultIntegerKindV1,
        target_kind: DefaultIntegerKindV1,
        operand: Box<IndexedDefaultExpressionV1<'a>>,
    },
    Binary {
        operator: DefaultBinaryOperatorV1,
        lhs: Box<IndexedDefaultExpressionV1<'a>>,
        rhs: Box<IndexedDefaultExpressionV1<'a>>,
    },
    Unary {
        operator: DefaultUnaryOperatorV1,
        operand: Box<IndexedDefaultExpressionV1<'a>>,
    },
    SomeWrap(Box<IndexedDefaultExpressionV1<'a>>),
    NoneLiteral,
    IsSome(Box<IndexedDefaultExpressionV1<'a>>),
    Unwrap {
        operand: Box<IndexedDefaultExpressionV1<'a>>,
        trap_on_none: CanonicalBooleanV1,
    },
}

#[derive(Debug)]
enum IndexedOptionalDefaultExpressionV1<'a> {
    Absent,
    Present(Box<IndexedDefaultExpressionV1<'a>>),
}

#[derive(Debug)]
struct IndexedDefaultArrayAssemblyV1<'a> {
    element_type: &'a SignatureTypeKey,
    parts: Vec<IndexedDefaultArrayAssemblyPartV1<'a>>,
    result_type: &'a SignatureTypeKey,
}

#[derive(Debug)]
enum IndexedDefaultArrayAssemblyPartV1<'a> {
    Element(IndexedDefaultExpressionV1<'a>),
    CopyArray(IndexedDefaultExpressionV1<'a>),
}

#[derive(Debug)]
enum IndexedDefaultIntegerArgumentsV1<'a> {
    Unary(Box<IndexedDefaultExpressionV1<'a>>),
    Binary {
        lhs: Box<IndexedDefaultExpressionV1<'a>>,
        rhs: Box<IndexedDefaultExpressionV1<'a>>,
    },
}

impl DefaultExpressionV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultExpressionV1<'_>, DefaultExpressionIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let kind = match &self.kind {
            DefaultExpressionKindV1::StringLiteral { value, owner } => {
                IndexedDefaultExpressionKindV1::StringLiteral { value, owner }
            }
            DefaultExpressionKindV1::IntegerLiteral(value) => {
                IndexedDefaultExpressionKindV1::IntegerLiteral(value)
            }
            DefaultExpressionKindV1::BooleanLiteral(value) => {
                IndexedDefaultExpressionKindV1::BooleanLiteral(*value)
            }
            DefaultExpressionKindV1::UnitLiteral => IndexedDefaultExpressionKindV1::UnitLiteral,
            DefaultExpressionKindV1::TupleLiteral(elements) => {
                IndexedDefaultExpressionKindV1::TupleLiteral(index_sequence(
                    elements, resolver, 5, 1,
                )?)
            }
            DefaultExpressionKindV1::StructInit {
                constructor,
                arguments,
            } => IndexedDefaultExpressionKindV1::StructInit {
                constructor,
                arguments: index_sequence(arguments, resolver, 6, 2)?,
            },
            DefaultExpressionKindV1::StructConstruct { owner_type, fields } => {
                IndexedDefaultExpressionKindV1::StructConstruct {
                    owner_type,
                    fields: index_sequence(fields, resolver, 7, 2)?,
                }
            }
            DefaultExpressionKindV1::ClassInit {
                constructor,
                arguments,
            } => IndexedDefaultExpressionKindV1::ClassInit {
                constructor,
                arguments: index_sequence(arguments, resolver, 8, 2)?,
            },
            DefaultExpressionKindV1::VariantConstruct { variant, arguments } => {
                IndexedDefaultExpressionKindV1::VariantConstruct {
                    variant,
                    arguments: index_sequence(arguments, resolver, 9, 2)?,
                }
            }
            DefaultExpressionKindV1::VariantTest { operand, variant } => {
                IndexedDefaultExpressionKindV1::VariantTest {
                    operand: index_child(operand, resolver, 10, 1)?,
                    variant,
                }
            }
            DefaultExpressionKindV1::VariantPayloadProject { operand, field } => {
                IndexedDefaultExpressionKindV1::VariantPayloadProject {
                    operand: index_child(operand, resolver, 11, 1)?,
                    field,
                }
            }
            DefaultExpressionKindV1::Capture(index) => {
                IndexedDefaultExpressionKindV1::Capture(*index)
            }
            DefaultExpressionKindV1::Local(local) => IndexedDefaultExpressionKindV1::Local(
                resolver
                    .resolve_template_local_index(local)
                    .map_err(DefaultExpressionIndexError::Local)?,
            ),
            DefaultExpressionKindV1::GlobalRead(property) => {
                IndexedDefaultExpressionKindV1::GlobalRead(property)
            }
            DefaultExpressionKindV1::GenericDelegateStorageRead(reference) => {
                IndexedDefaultExpressionKindV1::GenericDelegateStorageRead(reference)
            }
            DefaultExpressionKindV1::SingletonValue(value) => {
                IndexedDefaultExpressionKindV1::SingletonValue(value)
            }
            DefaultExpressionKindV1::Lambda(lambda) => IndexedDefaultExpressionKindV1::Lambda(
                lambda
                    .index_locals(resolver)
                    .map_err(DefaultExpressionIndexError::Lambda)?,
            ),
            DefaultExpressionKindV1::AnonymousFunction(function) => {
                IndexedDefaultExpressionKindV1::AnonymousFunction(
                    function
                        .index_locals(resolver)
                        .map_err(DefaultExpressionIndexError::AnonymousFunction)?,
                )
            }
            DefaultExpressionKindV1::CallableReference(reference) => {
                IndexedDefaultExpressionKindV1::CallableReference(
                    reference.index_locals(resolver).map_err(|error| {
                        DefaultExpressionIndexError::CallableReference(Box::new(error))
                    })?,
                )
            }
            DefaultExpressionKindV1::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => IndexedDefaultExpressionKindV1::FunctionCoercion {
                source: index_child(source, resolver, 18, 1)?,
                source_function_type,
                target_function_type,
            },
            DefaultExpressionKindV1::PtrFromNonZeroULong(operand) => {
                IndexedDefaultExpressionKindV1::PtrFromNonZeroULong(index_child(
                    operand, resolver, 19, 1,
                )?)
            }
            DefaultExpressionKindV1::PtrToULong(operand) => {
                IndexedDefaultExpressionKindV1::PtrToULong(index_child(operand, resolver, 20, 1)?)
            }
            DefaultExpressionKindV1::PtrCast(operand) => {
                IndexedDefaultExpressionKindV1::PtrCast(index_child(operand, resolver, 21, 1)?)
            }
            DefaultExpressionKindV1::PtrLoad { pointer, offset } => {
                IndexedDefaultExpressionKindV1::PtrLoad {
                    pointer: index_child(pointer, resolver, 22, 1)?,
                    offset: index_optional(offset, resolver, 22, 2)?,
                }
            }
            DefaultExpressionKindV1::PtrStore {
                pointer,
                offset,
                value,
            } => IndexedDefaultExpressionKindV1::PtrStore {
                pointer: index_child(pointer, resolver, 23, 1)?,
                offset: index_optional(offset, resolver, 23, 2)?,
                value: index_child(value, resolver, 23, 3)?,
            },
            DefaultExpressionKindV1::PtrOffset {
                pointer,
                offset,
                subtract,
            } => IndexedDefaultExpressionKindV1::PtrOffset {
                pointer: index_child(pointer, resolver, 24, 1)?,
                offset: index_child(offset, resolver, 24, 2)?,
                subtract: *subtract,
            },
            DefaultExpressionKindV1::AddressOf(place) => IndexedDefaultExpressionKindV1::AddressOf(
                place
                    .index_local(resolver)
                    .map_err(DefaultExpressionIndexError::Place)?,
            ),
            DefaultExpressionKindV1::SizeOf(queried_type) => {
                IndexedDefaultExpressionKindV1::SizeOf(queried_type)
            }
            DefaultExpressionKindV1::AlignOf(queried_type) => {
                IndexedDefaultExpressionKindV1::AlignOf(queried_type)
            }
            DefaultExpressionKindV1::FunctionAddress(declaration) => {
                IndexedDefaultExpressionKindV1::FunctionAddress(declaration)
            }
            DefaultExpressionKindV1::ForeignCallbackRegister {
                registration,
                closure,
            } => IndexedDefaultExpressionKindV1::ForeignCallbackRegister {
                registration,
                closure: index_child(closure, resolver, 29, 2)?,
            },
            DefaultExpressionKindV1::ForeignCallbackOperation {
                operation,
                callback,
            } => IndexedDefaultExpressionKindV1::ForeignCallbackOperation {
                operation: *operation,
                callback: index_child(callback, resolver, 30, 2)?,
            },
            DefaultExpressionKindV1::ReleaseFieldLoad {
                owner_type,
                declaration,
            } => IndexedDefaultExpressionKindV1::ReleaseFieldLoad {
                owner_type,
                declaration,
            },
            DefaultExpressionKindV1::FieldAccess { receiver, field } => {
                IndexedDefaultExpressionKindV1::FieldAccess {
                    receiver: index_child(receiver, resolver, 31, 1)?,
                    field,
                }
            }
            DefaultExpressionKindV1::MethodCall {
                receiver,
                callee,
                arguments,
            } => IndexedDefaultExpressionKindV1::MethodCall {
                receiver: index_child(receiver, resolver, 32, 1)?,
                callee,
                arguments: index_sequence(arguments, resolver, 32, 3)?,
            },
            DefaultExpressionKindV1::DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => IndexedDefaultExpressionKindV1::DirectSuperMethodCall {
                receiver: index_child(receiver, resolver, 33, 1)?,
                callee,
                arguments: index_sequence(arguments, resolver, 33, 3)?,
            },
            DefaultExpressionKindV1::Box(operand) => {
                IndexedDefaultExpressionKindV1::Box(index_child(operand, resolver, 34, 1)?)
            }
            DefaultExpressionKindV1::Unbox(operand) => {
                IndexedDefaultExpressionKindV1::Unbox(index_child(operand, resolver, 35, 1)?)
            }
            DefaultExpressionKindV1::ReferenceUpcast(operand) => {
                IndexedDefaultExpressionKindV1::ReferenceUpcast(index_child(
                    operand, resolver, 58, 1,
                )?)
            }
            DefaultExpressionKindV1::IsInstance {
                operand,
                checked_type,
            } => IndexedDefaultExpressionKindV1::IsInstance {
                operand: index_child(operand, resolver, 36, 1)?,
                checked_type,
            },
            DefaultExpressionKindV1::Cast {
                operand,
                checked_type,
                optional,
            } => IndexedDefaultExpressionKindV1::Cast {
                operand: index_child(operand, resolver, 37, 1)?,
                checked_type,
                optional: *optional,
            },
            DefaultExpressionKindV1::ArrayLiteral(elements) => {
                IndexedDefaultExpressionKindV1::ArrayLiteral(index_sequence(
                    elements, resolver, 38, 1,
                )?)
            }
            DefaultExpressionKindV1::ArrayGenerate { count, initializer } => {
                IndexedDefaultExpressionKindV1::ArrayGenerate {
                    count: index_child(count, resolver, 62, 1)?,
                    initializer: index_child(initializer, resolver, 62, 2)?,
                }
            }
            DefaultExpressionKindV1::ArrayAssembly(assembly) => {
                IndexedDefaultExpressionKindV1::ArrayAssembly(index_array_assembly(
                    assembly, resolver,
                )?)
            }
            DefaultExpressionKindV1::Index {
                access,
                receiver,
                index,
            } => IndexedDefaultExpressionKindV1::Index {
                access: *access,
                receiver: index_child(receiver, resolver, 40, 2)?,
                index: index_child(index, resolver, 40, 3)?,
            },
            DefaultExpressionKindV1::ArraySet {
                access,
                receiver,
                index,
                value,
            } => IndexedDefaultExpressionKindV1::ArraySet {
                access: *access,
                receiver: index_child(receiver, resolver, 41, 2)?,
                index: index_child(index, resolver, 41, 3)?,
                value: index_child(value, resolver, 41, 4)?,
            },
            DefaultExpressionKindV1::ArrayLen(operand) => {
                IndexedDefaultExpressionKindV1::ArrayLen(index_child(operand, resolver, 42, 1)?)
            }
            DefaultExpressionKindV1::ArrayClone(operand) => {
                IndexedDefaultExpressionKindV1::ArrayClone(index_child(operand, resolver, 43, 1)?)
            }
            DefaultExpressionKindV1::Call {
                callee,
                arguments,
                receiver,
            } => IndexedDefaultExpressionKindV1::Call {
                callee,
                arguments: index_sequence(arguments, resolver, 57, 2)?,
                receiver,
            },
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => IndexedDefaultExpressionKindV1::LocalFunctionCall {
                declaration,
                callee,
                captures: index_sequence(captures, resolver, 45, 3)?,
                arguments: index_sequence(arguments, resolver, 45, 4)?,
            },
            DefaultExpressionKindV1::CallableCall {
                callee,
                function_type,
                arguments,
            } => IndexedDefaultExpressionKindV1::CallableCall {
                callee: index_child(callee, resolver, 46, 1)?,
                function_type,
                arguments: index_sequence(arguments, resolver, 46, 3)?,
            },
            DefaultExpressionKindV1::PrimitiveBinary { kind, lhs, rhs } => {
                IndexedDefaultExpressionKindV1::PrimitiveBinary {
                    kind: *kind,
                    lhs: index_child(lhs, resolver, 47, 2)?,
                    rhs: index_child(rhs, resolver, 47, 3)?,
                }
            }
            DefaultExpressionKindV1::PrimitiveUnary { kind, operand } => {
                IndexedDefaultExpressionKindV1::PrimitiveUnary {
                    kind: *kind,
                    operand: index_child(operand, resolver, 48, 2)?,
                }
            }
            DefaultExpressionKindV1::IntegerOperation {
                operation,
                arguments,
            } => IndexedDefaultExpressionKindV1::IntegerOperation {
                operation,
                arguments: index_integer_arguments(arguments, resolver)?,
            },
            DefaultExpressionKindV1::IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => IndexedDefaultExpressionKindV1::IntegerConversion {
                source_kind: *source_kind,
                target_kind: *target_kind,
                operand: index_child(operand, resolver, 50, 3)?,
            },
            DefaultExpressionKindV1::Binary { operator, lhs, rhs } => {
                IndexedDefaultExpressionKindV1::Binary {
                    operator: *operator,
                    lhs: index_child(lhs, resolver, 51, 2)?,
                    rhs: index_child(rhs, resolver, 51, 3)?,
                }
            }
            DefaultExpressionKindV1::Unary { operator, operand } => {
                IndexedDefaultExpressionKindV1::Unary {
                    operator: *operator,
                    operand: index_child(operand, resolver, 52, 2)?,
                }
            }
            DefaultExpressionKindV1::SomeWrap(operand) => {
                IndexedDefaultExpressionKindV1::SomeWrap(index_child(operand, resolver, 53, 1)?)
            }
            DefaultExpressionKindV1::NoneLiteral => IndexedDefaultExpressionKindV1::NoneLiteral,
            DefaultExpressionKindV1::IsSome(operand) => {
                IndexedDefaultExpressionKindV1::IsSome(index_child(operand, resolver, 55, 1)?)
            }
            DefaultExpressionKindV1::Unwrap {
                operand,
                trap_on_none,
            } => IndexedDefaultExpressionKindV1::Unwrap {
                operand: index_child(operand, resolver, 56, 1)?,
                trap_on_none: *trap_on_none,
            },
        };
        Ok(IndexedDefaultExpressionV1 {
            expression: self,
            kind,
        })
    }
}

mod wire;

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultExpressionIndexError<E> {
    Local(E),
    Place(DefaultPlaceIndexError<E>),
    Lambda(DefaultLexicalCallableIndexError<E>),
    AnonymousFunction(DefaultLexicalCallableIndexError<E>),
    CallableReference(Box<DefaultCallableReferenceIndexError<E>>),
    Nested {
        variant_tag: u64,
        field: u32,
        index: Option<usize>,
        error: Box<Self>,
    },
}

impl<E: fmt::Display> fmt::Display for DefaultExpressionIndexError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => {
                write!(formatter, "cannot index default expression local: {error}")
            }
            Self::Place(error) => write!(formatter, "cannot index default address place: {error}"),
            Self::Lambda(error) => write!(formatter, "cannot index default lambda: {error}"),
            Self::AnonymousFunction(error) => {
                write!(
                    formatter,
                    "cannot index default anonymous function: {error}"
                )
            }
            Self::CallableReference(error) => {
                write!(
                    formatter,
                    "cannot index default callable reference: {error}"
                )
            }
            Self::Nested {
                variant_tag,
                field,
                index: Some(index),
                error,
            } => write!(
                formatter,
                "cannot index default expression tag {variant_tag} field {field} element {index}: {error}"
            ),
            Self::Nested {
                variant_tag,
                field,
                index: None,
                error,
            } => write!(
                formatter,
                "cannot index default expression tag {variant_tag} field {field}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultExpressionIndexError<E> {}

fn index_child<'a, I>(
    expression: &'a DefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<Box<IndexedDefaultExpressionV1<'a>>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    expression
        .index_locals(resolver)
        .map(Box::new)
        .map_err(|error| DefaultExpressionIndexError::Nested {
            variant_tag,
            field,
            index: None,
            error: Box::new(error),
        })
}

fn index_sequence<'a, I>(
    expressions: &'a [DefaultExpressionV1],
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<Vec<IndexedDefaultExpressionV1<'a>>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut indexed = Vec::with_capacity(expressions.len());
    for (index, expression) in expressions.iter().enumerate() {
        indexed.push(expression.index_locals(resolver).map_err(|error| {
            DefaultExpressionIndexError::Nested {
                variant_tag,
                field,
                index: Some(index),
                error: Box::new(error),
            }
        })?);
    }
    Ok(indexed)
}

fn index_optional<'a, I>(
    expression: &'a OptionalDefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
) -> Result<IndexedOptionalDefaultExpressionV1<'a>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    match expression {
        OptionalDefaultExpressionV1::Absent => Ok(IndexedOptionalDefaultExpressionV1::Absent),
        OptionalDefaultExpressionV1::Present(expression) => {
            index_child(expression, resolver, variant_tag, field)
                .map(IndexedOptionalDefaultExpressionV1::Present)
        }
    }
}

fn index_array_assembly<'a, I>(
    assembly: &'a DefaultArrayAssemblyV1,
    resolver: &mut I,
) -> Result<IndexedDefaultArrayAssemblyV1<'a>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    let mut parts = Vec::with_capacity(assembly.parts.len());
    for (index, part) in assembly.parts.iter().enumerate() {
        let indexed = match part {
            DefaultArrayAssemblyPartV1::Element(expression) => {
                IndexedDefaultArrayAssemblyPartV1::Element(index_sequence_element(
                    expression, resolver, 39, 2, index,
                )?)
            }
            DefaultArrayAssemblyPartV1::CopyArray(expression) => {
                IndexedDefaultArrayAssemblyPartV1::CopyArray(index_sequence_element(
                    expression, resolver, 39, 2, index,
                )?)
            }
        };
        parts.push(indexed);
    }
    Ok(IndexedDefaultArrayAssemblyV1 {
        element_type: &assembly.element_type,
        parts,
        result_type: &assembly.result_type,
    })
}

fn index_integer_arguments<'a, I>(
    arguments: &'a DefaultIntegerArgumentsV1,
    resolver: &mut I,
) -> Result<IndexedDefaultIntegerArgumentsV1<'a>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    match arguments {
        DefaultIntegerArgumentsV1::Unary(operand) => {
            index_child(operand, resolver, 49, 2).map(IndexedDefaultIntegerArgumentsV1::Unary)
        }
        DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
            Ok(IndexedDefaultIntegerArgumentsV1::Binary {
                lhs: index_sequence_child(lhs, resolver, 49, 2, 0)?,
                rhs: index_sequence_child(rhs, resolver, 49, 2, 1)?,
            })
        }
    }
}

fn index_sequence_element<'a, I>(
    expression: &'a DefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
    index: usize,
) -> Result<IndexedDefaultExpressionV1<'a>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    expression
        .index_locals(resolver)
        .map_err(|error| DefaultExpressionIndexError::Nested {
            variant_tag,
            field,
            index: Some(index),
            error: Box::new(error),
        })
}

fn index_sequence_child<'a, I>(
    expression: &'a DefaultExpressionV1,
    resolver: &mut I,
    variant_tag: u64,
    field: u32,
    index: usize,
) -> Result<Box<IndexedDefaultExpressionV1<'a>>, DefaultExpressionIndexError<I::Error>>
where
    I: TemplateLocalIndexResolver,
{
    index_sequence_element(expression, resolver, variant_tag, field, index).map(Box::new)
}
