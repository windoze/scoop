use scoop_identity::{
    ConeIdentity, DecodedCallableTemplateOrigin, DecodedPersistentId, DecodedSignatureTypeKey,
    PersistentCallbackRegistrationId, PersistentIdResolver, PersistentKeyResolver,
    PersistentObjectValueId, PersistentPropertyId, PersistentSourceContextId, SourceContextKey,
    SourceOriginResolutionError,
};

use super::{DefaultArrayAssemblyBuildError, DefaultExpressionBuildError};
use crate::{
    CanonicalBooleanV1, CanonicalIntegerConstantV1, DecodedDefaultAnonymousFunctionV1,
    DecodedDefaultCallableDeclarationV1, DecodedDefaultCallableRefV1,
    DecodedDefaultConstructorRefV1, DecodedDefaultEnumVariantFieldRefV1,
    DecodedDefaultEnumVariantRefV1, DecodedDefaultFieldRefV1, DecodedDefaultIntegerOperationV1,
    DecodedDefaultLambdaV1, DecodedDefaultMethodCalleeV1, DecodedDefaultPlaceV1,
    DecodedDefaultStringOwnerV1, DecodedExportDefinitionSourceV1, DefaultArrayAccessKindV1,
    DefaultBinaryOperatorV1, DefaultCallableRefResolutionError,
    DefaultConstructorRefResolutionError, DefaultConstructorReferenceResolver,
    DefaultEnumVariantFieldRefResolutionError, DefaultEnumVariantRefResolutionError,
    DefaultFieldRefResolutionError, DefaultFieldReferenceResolver,
    DefaultForeignCallbackOperationV1, DefaultIntegerKindV1, DefaultLexicalCallableResolutionError,
    DefaultMethodCalleeResolutionError, DefaultNestedCallableReferenceResolver,
    DefaultPlaceResolutionError, DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1,
    DefaultStringOwnerResolutionError, DefaultUnaryOperatorV1,
};

use super::super::{DecodedDefaultCallableReferenceV1, DefaultCallableReferenceResolutionError};

mod resolve;
mod wire;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultExpressionV1 {
    kind: DecodedDefaultExpressionKindV1,
    result_type: DecodedSignatureTypeKey,
    definition_origin: DecodedExportDefinitionSourceV1,
    evaluation_origin: scoop_identity::DecodedEvaluationOrigin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDefaultExpressionKindV1 {
    ContextLookup {
        declaration: DecodedDefaultCallableDeclarationV1,
        parameter: crate::ContextParameterIndex,
        diagnostic: crate::ContextDiagnostic,
    },
    StringLiteral {
        value: String,
        owner: DecodedDefaultStringOwnerV1,
    },
    IntegerLiteral(CanonicalIntegerConstantV1),
    BooleanLiteral(CanonicalBooleanV1),
    CharLiteral(crate::CanonicalCharV1),
    FloatLiteral(scoop_identity::FloatConstant),
    UnitLiteral,
    TupleLiteral(Vec<DecodedDefaultExpressionV1>),
    StructInit {
        constructor: DecodedDefaultConstructorRefV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    StructConstruct {
        owner_type: DecodedSignatureTypeKey,
        fields: Vec<DecodedDefaultExpressionV1>,
    },
    ClassInit {
        constructor: DecodedDefaultConstructorRefV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    VariantConstruct {
        variant: DecodedDefaultEnumVariantRefV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    VariantTest {
        operand: Box<DecodedDefaultExpressionV1>,
        variant: DecodedDefaultEnumVariantRefV1,
    },
    VariantPayloadProject {
        operand: Box<DecodedDefaultExpressionV1>,
        field: DecodedDefaultEnumVariantFieldRefV1,
    },
    Local(u32),
    Capture(u32),
    GlobalRead(DecodedPersistentId<PersistentPropertyId>),
    GenericDelegateStorageRead(crate::DecodedDefaultGenericDelegateReferenceV1),
    SingletonValue(DecodedPersistentId<PersistentObjectValueId>),
    Lambda(DecodedDefaultLambdaV1),
    AnonymousFunction(DecodedDefaultAnonymousFunctionV1),
    CallableReference(DecodedDefaultCallableReferenceV1),
    FunctionCoercion {
        source: Box<DecodedDefaultExpressionV1>,
        source_function_type: DecodedSignatureTypeKey,
        target_function_type: DecodedSignatureTypeKey,
    },
    PtrFromNonZeroULong(Box<DecodedDefaultExpressionV1>),
    PtrToULong(Box<DecodedDefaultExpressionV1>),
    CharCode(Box<DecodedDefaultExpressionV1>),
    CharFromCodeUnchecked(Box<DecodedDefaultExpressionV1>),
    PtrCast(Box<DecodedDefaultExpressionV1>),
    PtrLoad {
        pointer: Box<DecodedDefaultExpressionV1>,
        offset: DecodedOptionalDefaultExpressionV1,
    },
    PtrStore {
        pointer: Box<DecodedDefaultExpressionV1>,
        offset: DecodedOptionalDefaultExpressionV1,
        value: Box<DecodedDefaultExpressionV1>,
    },
    PtrOffset {
        pointer: Box<DecodedDefaultExpressionV1>,
        offset: Box<DecodedDefaultExpressionV1>,
        subtract: CanonicalBooleanV1,
    },
    AddressOf(DecodedDefaultPlaceV1),
    SizeOf(DecodedSignatureTypeKey),
    AlignOf(DecodedSignatureTypeKey),
    FunctionAddress(DecodedDefaultCallableDeclarationV1),
    ForeignCallbackRegister {
        registration: DecodedPersistentId<PersistentCallbackRegistrationId>,
        closure: Box<DecodedDefaultExpressionV1>,
    },
    ForeignCallbackOperation {
        operation: DefaultForeignCallbackOperationV1,
        callback: Box<DecodedDefaultExpressionV1>,
    },
    ReleaseFieldLoad {
        owner_type: DecodedSignatureTypeKey,
        declaration: DecodedPersistentId<scoop_identity::PersistentFieldId>,
    },
    FieldAccess {
        receiver: Box<DecodedDefaultExpressionV1>,
        field: DecodedDefaultFieldRefV1,
    },
    MethodCall {
        receiver: Box<DecodedDefaultExpressionV1>,
        callee: DecodedDefaultMethodCalleeV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    DirectSuperMethodCall {
        receiver: Box<DecodedDefaultExpressionV1>,
        callee: DecodedDefaultMethodCalleeV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    Box(Box<DecodedDefaultExpressionV1>),
    Unbox(Box<DecodedDefaultExpressionV1>),
    ReferenceUpcast(Box<DecodedDefaultExpressionV1>),
    IsInstance {
        operand: Box<DecodedDefaultExpressionV1>,
        checked_type: DecodedSignatureTypeKey,
    },
    Cast {
        operand: Box<DecodedDefaultExpressionV1>,
        checked_type: DecodedSignatureTypeKey,
        optional: CanonicalBooleanV1,
    },
    ArrayLiteral(Vec<DecodedDefaultExpressionV1>),
    ArrayGenerate {
        count: Box<DecodedDefaultExpressionV1>,
        initializer: Box<DecodedDefaultExpressionV1>,
    },
    ArrayAssembly(DecodedDefaultArrayAssemblyV1),
    Index {
        access: DefaultArrayAccessKindV1,
        receiver: Box<DecodedDefaultExpressionV1>,
        index: Box<DecodedDefaultExpressionV1>,
    },
    ArraySet {
        access: DefaultArrayAccessKindV1,
        receiver: Box<DecodedDefaultExpressionV1>,
        index: Box<DecodedDefaultExpressionV1>,
        value: Box<DecodedDefaultExpressionV1>,
    },
    ArrayLen(Box<DecodedDefaultExpressionV1>),
    ArrayClone(Box<DecodedDefaultExpressionV1>),
    Call {
        callee: DecodedDefaultCallableRefV1,
        arguments: Vec<DecodedDefaultExpressionV1>,
        receiver: crate::SourceCallReceiver<DecodedSignatureTypeKey>,
    },
    LocalFunctionCall {
        declaration: DecodedCallableTemplateOrigin,
        callee: DecodedDefaultCallableRefV1,
        captures: Vec<DecodedDefaultExpressionV1>,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    CallableCall {
        callee: Box<DecodedDefaultExpressionV1>,
        function_type: DecodedSignatureTypeKey,
        arguments: Vec<DecodedDefaultExpressionV1>,
    },
    PrimitiveBinary {
        kind: DefaultPrimitiveBinaryKindV1,
        lhs: Box<DecodedDefaultExpressionV1>,
        rhs: Box<DecodedDefaultExpressionV1>,
    },
    PrimitiveUnary {
        kind: DefaultPrimitiveUnaryKindV1,
        operand: Box<DecodedDefaultExpressionV1>,
    },
    IntegerOperation {
        operation: DecodedDefaultIntegerOperationV1,
        arguments: DecodedDefaultIntegerArgumentsV1,
    },
    IntegerConversion {
        source_kind: DefaultIntegerKindV1,
        target_kind: DefaultIntegerKindV1,
        operand: Box<DecodedDefaultExpressionV1>,
    },
    Binary {
        operator: DefaultBinaryOperatorV1,
        lhs: Box<DecodedDefaultExpressionV1>,
        rhs: Box<DecodedDefaultExpressionV1>,
    },
    Unary {
        operator: DefaultUnaryOperatorV1,
        operand: Box<DecodedDefaultExpressionV1>,
    },
    SomeWrap(Box<DecodedDefaultExpressionV1>),
    NoneLiteral,
    IsSome(Box<DecodedDefaultExpressionV1>),
    Unwrap {
        operand: Box<DecodedDefaultExpressionV1>,
        trap_on_none: CanonicalBooleanV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOptionalDefaultExpressionV1 {
    Absent,
    Present(Box<DecodedDefaultExpressionV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultArrayAssemblyV1 {
    element_type: DecodedSignatureTypeKey,
    parts: Vec<DecodedDefaultArrayAssemblyPartV1>,
    result_type: DecodedSignatureTypeKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultArrayAssemblyPartV1 {
    Element(DecodedDefaultExpressionV1),
    CopyArray(DecodedDefaultExpressionV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultIntegerArgumentsV1 {
    Unary(Box<DecodedDefaultExpressionV1>),
    Binary {
        lhs: Box<DecodedDefaultExpressionV1>,
        rhs: Box<DecodedDefaultExpressionV1>,
    },
}

pub trait DefaultExpressionReferenceResolver<E>:
    DefaultNestedCallableReferenceResolver<E>
    + DefaultConstructorReferenceResolver<E>
    + DefaultFieldReferenceResolver<E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentObjectValueId, Error = E>
    + PersistentIdResolver<PersistentCallbackRegistrationId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> DefaultExpressionReferenceResolver<E> for R where
    R: DefaultNestedCallableReferenceResolver<E>
        + DefaultConstructorReferenceResolver<E>
        + DefaultFieldReferenceResolver<E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentObjectValueId, Error = E>
        + PersistentIdResolver<PersistentCallbackRegistrationId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultExpressionResolutionError<E, L> {
    Local(L),
    Persistent {
        variant_tag: u64,
        field: u32,
        error: E,
    },
    Type {
        variant_tag: u64,
        field: u32,
        error: E,
    },
    StringOwner(DefaultStringOwnerResolutionError<E>),
    Constructor {
        variant_tag: u64,
        error: DefaultConstructorRefResolutionError<E>,
    },
    Variant {
        variant_tag: u64,
        error: DefaultEnumVariantRefResolutionError<E>,
    },
    VariantField(DefaultEnumVariantFieldRefResolutionError<E>),
    Lambda(DefaultLexicalCallableResolutionError<E, L>),
    AnonymousFunction(DefaultLexicalCallableResolutionError<E, L>),
    CallableReference(Box<DefaultCallableReferenceResolutionError<E, L>>),
    Place(DefaultPlaceResolutionError<E, L>),
    CallableDeclaration(E),
    Field(DefaultFieldRefResolutionError<E>),
    Method {
        variant_tag: u64,
        error: DefaultMethodCalleeResolutionError<E>,
    },
    Callable {
        variant_tag: u64,
        field: u32,
        error: DefaultCallableRefResolutionError<E>,
    },
    DefinitionOrigin(SourceOriginResolutionError<E>),
    EvaluationOrigin(SourceOriginResolutionError<E>),
    Nested {
        variant_tag: u64,
        field: u32,
        index: Option<usize>,
        error: Box<Self>,
    },
    Record(DefaultExpressionBuildError),
    ArrayAssembly(DefaultArrayAssemblyBuildError),
}
