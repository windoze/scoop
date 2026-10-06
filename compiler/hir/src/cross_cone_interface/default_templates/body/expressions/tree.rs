use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, LocalValueSelector, PersistentCallbackRegistrationId,
    PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey,
};

use super::{DefaultCallableReferenceV1, DefaultIntegerOperationV1, DefaultStringOwnerV1};
use crate::{
    CanonicalBooleanV1, CanonicalIntegerConstantV1, DefaultAnonymousFunctionV1,
    DefaultArrayAccessKindV1, DefaultBinaryOperatorV1, DefaultCallableDeclarationV1,
    DefaultCallableRefV1, DefaultConstructorRefV1, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefV1, DefaultFieldRefV1, DefaultForeignCallbackOperationV1, DefaultLambdaV1,
    DefaultMethodCalleeV1, DefaultPlaceV1, DefaultPrimitiveBinaryKindV1,
    DefaultPrimitiveUnaryKindV1, DefaultUnaryOperatorV1,
};

mod decoded;
mod indexed;

pub use decoded::{
    DecodedDefaultArrayAssemblyPartV1, DecodedDefaultArrayAssemblyV1, DecodedDefaultExpressionV1,
    DecodedDefaultIntegerArgumentsV1, DecodedOptionalDefaultExpressionV1,
    DefaultExpressionReferenceResolver, DefaultExpressionResolutionError,
};
pub use indexed::{DefaultExpressionIndexError, IndexedDefaultExpressionV1};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultExpressionV1 {
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
    definition_origin: crate::ExportDefinitionSourceV1,
    evaluation_origin: scoop_identity::EvaluationOrigin,
}

impl DefaultExpressionV1 {
    pub fn try_new(
        kind: DefaultExpressionKindV1,
        result_type: SignatureTypeKey,
        definition_origin: crate::ExportDefinitionSourceV1,
        evaluation_origin: scoop_identity::EvaluationOrigin,
    ) -> Result<Self, DefaultExpressionBuildError> {
        validate_kind(&kind)?;
        Ok(Self {
            kind,
            result_type,
            definition_origin,
            evaluation_origin,
        })
    }

    pub const fn kind(&self) -> &DefaultExpressionKindV1 {
        &self.kind
    }

    pub const fn result_type(&self) -> &SignatureTypeKey {
        &self.result_type
    }

    pub const fn definition_origin(&self) -> &crate::ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub const fn evaluation_origin(&self) -> &scoop_identity::EvaluationOrigin {
        &self.evaluation_origin
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultExpressionKindV1 {
    ContextLookup {
        declaration: DefaultCallableDeclarationV1,
        parameter: crate::ContextParameterIndex,
        diagnostic: crate::ContextDiagnostic,
    },
    ReleaseFieldLoad {
        owner_type: SignatureTypeKey,
        declaration: scoop_identity::PersistentFieldId,
    },
    StringLiteral {
        value: String,
        owner: DefaultStringOwnerV1,
    },
    IntegerLiteral(CanonicalIntegerConstantV1),
    BooleanLiteral(CanonicalBooleanV1),
    CharLiteral(crate::CanonicalCharV1),
    FloatLiteral(scoop_identity::FloatConstant),
    FloatUnary {
        kind: crate::FloatKind,
        operation: crate::FloatUnaryOperator,
        operand: Box<DefaultExpressionV1>,
    },
    FloatBinary {
        kind: crate::FloatKind,
        operation: crate::FloatBinaryOperator,
        lhs: Box<DefaultExpressionV1>,
        rhs: Box<DefaultExpressionV1>,
    },
    FloatConversion {
        conversion: crate::DefaultFloatConversionV1,
        operand: Box<DefaultExpressionV1>,
    },
    UnitLiteral,
    TupleLiteral(Vec<DefaultExpressionV1>),
    StructInit {
        constructor: DefaultConstructorRefV1,
        arguments: Vec<DefaultExpressionV1>,
    },
    StructConstruct {
        owner_type: SignatureTypeKey,
        fields: Vec<DefaultExpressionV1>,
    },
    ClassInit {
        constructor: DefaultConstructorRefV1,
        arguments: Vec<DefaultExpressionV1>,
    },
    VariantConstruct {
        variant: DefaultEnumVariantRefV1,
        arguments: Vec<DefaultExpressionV1>,
    },
    VariantTest {
        operand: Box<DefaultExpressionV1>,
        variant: DefaultEnumVariantRefV1,
    },
    VariantPayloadProject {
        operand: Box<DefaultExpressionV1>,
        field: DefaultEnumVariantFieldRefV1,
    },
    Local(LocalValueSelector),
    /// Definition-order input in the current lexical closure body.
    Capture(u32),
    GlobalRead(PersistentPropertyId),
    GenericDelegateStorageRead(crate::DefaultGenericDelegateReferenceV1),
    SingletonValue(PersistentObjectValueId),
    Lambda(DefaultLambdaV1),
    AnonymousFunction(DefaultAnonymousFunctionV1),
    CallableReference(DefaultCallableReferenceV1),
    FunctionCoercion {
        source: Box<DefaultExpressionV1>,
        source_function_type: SignatureTypeKey,
        target_function_type: SignatureTypeKey,
    },
    PtrFromNonZeroULong(Box<DefaultExpressionV1>),
    PtrToULong(Box<DefaultExpressionV1>),
    CharCode(Box<DefaultExpressionV1>),
    CharFromCodeUnchecked(Box<DefaultExpressionV1>),
    PtrCast(Box<DefaultExpressionV1>),
    PtrLoad {
        pointer: Box<DefaultExpressionV1>,
        offset: OptionalDefaultExpressionV1,
    },
    PtrStore {
        pointer: Box<DefaultExpressionV1>,
        offset: OptionalDefaultExpressionV1,
        value: Box<DefaultExpressionV1>,
    },
    PtrOffset {
        pointer: Box<DefaultExpressionV1>,
        offset: Box<DefaultExpressionV1>,
        subtract: CanonicalBooleanV1,
    },
    AddressOf(DefaultPlaceV1),
    SizeOf(SignatureTypeKey),
    AlignOf(SignatureTypeKey),
    FunctionAddress(DefaultCallableDeclarationV1),
    ForeignCallbackRegister {
        registration: PersistentCallbackRegistrationId,
        closure: Box<DefaultExpressionV1>,
    },
    ForeignCallbackOperation {
        operation: DefaultForeignCallbackOperationV1,
        callback: Box<DefaultExpressionV1>,
    },
    FieldAccess {
        receiver: Box<DefaultExpressionV1>,
        field: DefaultFieldRefV1,
    },
    MethodCall {
        receiver: Box<DefaultExpressionV1>,
        callee: DefaultMethodCalleeV1,
        arguments: Vec<DefaultExpressionV1>,
    },
    DirectSuperMethodCall {
        receiver: Box<DefaultExpressionV1>,
        callee: DefaultMethodCalleeV1,
        arguments: Vec<DefaultExpressionV1>,
    },
    Box(Box<DefaultExpressionV1>),
    Unbox(Box<DefaultExpressionV1>),
    ReferenceUpcast(Box<DefaultExpressionV1>),
    IsInstance {
        operand: Box<DefaultExpressionV1>,
        checked_type: SignatureTypeKey,
    },
    Cast {
        operand: Box<DefaultExpressionV1>,
        checked_type: SignatureTypeKey,
        optional: CanonicalBooleanV1,
    },
    ArrayLiteral(Vec<DefaultExpressionV1>),
    ArrayGenerate {
        count: Box<DefaultExpressionV1>,
        initializer: Box<DefaultExpressionV1>,
    },
    ArrayAssembly(DefaultArrayAssemblyV1),
    Index {
        access: DefaultArrayAccessKindV1,
        receiver: Box<DefaultExpressionV1>,
        index: Box<DefaultExpressionV1>,
    },
    ArraySet {
        access: DefaultArrayAccessKindV1,
        receiver: Box<DefaultExpressionV1>,
        index: Box<DefaultExpressionV1>,
        value: Box<DefaultExpressionV1>,
    },
    ArrayLen(Box<DefaultExpressionV1>),
    ArrayClone(Box<DefaultExpressionV1>),
    Call {
        callee: DefaultCallableRefV1,
        arguments: Vec<DefaultExpressionV1>,
        receiver: crate::SourceCallReceiver<SignatureTypeKey>,
    },
    LocalFunctionCall {
        declaration: CallableTemplateOrigin,
        callee: DefaultCallableRefV1,
        captures: Vec<DefaultExpressionV1>,
        arguments: Vec<DefaultExpressionV1>,
    },
    CallableCall {
        callee: Box<DefaultExpressionV1>,
        function_type: SignatureTypeKey,
        arguments: Vec<DefaultExpressionV1>,
    },
    PrimitiveBinary {
        kind: DefaultPrimitiveBinaryKindV1,
        lhs: Box<DefaultExpressionV1>,
        rhs: Box<DefaultExpressionV1>,
    },
    PrimitiveUnary {
        kind: DefaultPrimitiveUnaryKindV1,
        operand: Box<DefaultExpressionV1>,
    },
    IntegerOperation {
        operation: DefaultIntegerOperationV1,
        arguments: DefaultIntegerArgumentsV1,
    },
    IntegerConversion {
        source_kind: crate::DefaultIntegerKindV1,
        target_kind: crate::DefaultIntegerKindV1,
        operand: Box<DefaultExpressionV1>,
    },
    Binary {
        operator: DefaultBinaryOperatorV1,
        lhs: Box<DefaultExpressionV1>,
        rhs: Box<DefaultExpressionV1>,
    },
    Unary {
        operator: DefaultUnaryOperatorV1,
        operand: Box<DefaultExpressionV1>,
    },
    SomeWrap(Box<DefaultExpressionV1>),
    NoneLiteral,
    IsSome(Box<DefaultExpressionV1>),
    Unwrap {
        operand: Box<DefaultExpressionV1>,
        trap_on_none: CanonicalBooleanV1,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalDefaultExpressionV1 {
    Absent,
    Present(Box<DefaultExpressionV1>),
}

impl OptionalDefaultExpressionV1 {
    pub const fn absent() -> Self {
        Self::Absent
    }

    pub fn present(expression: DefaultExpressionV1) -> Self {
        Self::Present(Box::new(expression))
    }

    pub fn as_ref(&self) -> Option<&DefaultExpressionV1> {
        match self {
            Self::Absent => None,
            Self::Present(expression) => Some(expression),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultArrayAssemblyV1 {
    element_type: SignatureTypeKey,
    parts: Vec<DefaultArrayAssemblyPartV1>,
    result_type: SignatureTypeKey,
}

impl DefaultArrayAssemblyV1 {
    pub fn try_new(
        element_type: SignatureTypeKey,
        parts: Vec<DefaultArrayAssemblyPartV1>,
        result_type: SignatureTypeKey,
    ) -> Result<Self, DefaultArrayAssemblyBuildError> {
        u32::try_from(parts.len()).map_err(|_| DefaultArrayAssemblyBuildError::TooManyParts)?;
        Ok(Self {
            element_type,
            parts,
            result_type,
        })
    }

    pub const fn element_type(&self) -> &SignatureTypeKey {
        &self.element_type
    }

    pub fn parts(&self) -> &[DefaultArrayAssemblyPartV1] {
        &self.parts
    }

    pub const fn result_type(&self) -> &SignatureTypeKey {
        &self.result_type
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultArrayAssemblyPartV1 {
    Element(DefaultExpressionV1),
    CopyArray(DefaultExpressionV1),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultIntegerArgumentsV1 {
    Unary(Box<DefaultExpressionV1>),
    Binary {
        lhs: Box<DefaultExpressionV1>,
        rhs: Box<DefaultExpressionV1>,
    },
}

impl DefaultIntegerArgumentsV1 {
    pub fn unary(operand: DefaultExpressionV1) -> Self {
        Self::Unary(Box::new(operand))
    }

    pub fn binary(lhs: DefaultExpressionV1, rhs: DefaultExpressionV1) -> Self {
        Self::Binary {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultExpressionBuildError {
    TooManyElements,
    TooManyArguments,
    TooManyFields,
    TooManyCaptures,
    MissingReceiverArgument,
    StructInitRequiresStructConstructor,
    ClassInitRequiresClassConstructor,
    UnsupportedLocalFunctionDeclaration(CallableTemplateOrigin),
}

impl fmt::Display for DefaultExpressionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyElements => {
                formatter.write_str("default expression element count exceeds u32")
            }
            Self::TooManyArguments => {
                formatter.write_str("default expression argument count exceeds u32")
            }
            Self::TooManyFields => formatter.write_str("default struct field count exceeds u32"),
            Self::TooManyCaptures => {
                formatter.write_str("default local-function capture count exceeds u32")
            }
            Self::MissingReceiverArgument => {
                formatter.write_str("default call receiver has no logical argument")
            }
            Self::StructInitRequiresStructConstructor => {
                formatter.write_str("default struct initialization requires a struct constructor")
            }
            Self::ClassInitRequiresClassConstructor => {
                formatter.write_str("default class initialization requires a class constructor")
            }
            Self::UnsupportedLocalFunctionDeclaration(declaration) => write!(
                formatter,
                "default local-function call uses unsupported declaration {declaration:?}"
            ),
        }
    }
}

impl std::error::Error for DefaultExpressionBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultArrayAssemblyBuildError {
    TooManyParts,
}

impl fmt::Display for DefaultArrayAssemblyBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyParts => {
                formatter.write_str("default array assembly part count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultArrayAssemblyBuildError {}

fn validate_kind(kind: &DefaultExpressionKindV1) -> Result<(), DefaultExpressionBuildError> {
    match kind {
        DefaultExpressionKindV1::TupleLiteral(elements)
        | DefaultExpressionKindV1::ArrayLiteral(elements) => {
            require_u32_len(elements.len(), DefaultExpressionBuildError::TooManyElements)?;
        }
        DefaultExpressionKindV1::StructInit {
            constructor,
            arguments,
        } => {
            if !matches!(constructor, DefaultConstructorRefV1::Struct { .. }) {
                return Err(DefaultExpressionBuildError::StructInitRequiresStructConstructor);
            }
            require_arguments(arguments)?;
        }
        DefaultExpressionKindV1::ClassInit {
            constructor,
            arguments,
        } => {
            if !matches!(constructor, DefaultConstructorRefV1::Class { .. }) {
                return Err(DefaultExpressionBuildError::ClassInitRequiresClassConstructor);
            }
            require_arguments(arguments)?;
        }
        DefaultExpressionKindV1::StructConstruct { fields, .. } => {
            require_u32_len(fields.len(), DefaultExpressionBuildError::TooManyFields)?;
        }
        DefaultExpressionKindV1::VariantConstruct { arguments, .. }
        | DefaultExpressionKindV1::MethodCall { arguments, .. }
        | DefaultExpressionKindV1::DirectSuperMethodCall { arguments, .. }
        | DefaultExpressionKindV1::CallableCall { arguments, .. } => {
            require_arguments(arguments)?;
        }
        DefaultExpressionKindV1::Call {
            arguments,
            receiver,
            ..
        } => {
            require_arguments(arguments)?;
            if receiver.has_receiver() && arguments.is_empty() {
                return Err(DefaultExpressionBuildError::MissingReceiverArgument);
            }
        }
        DefaultExpressionKindV1::LocalFunctionCall {
            declaration,
            captures,
            arguments,
            ..
        } => {
            if !matches!(
                declaration,
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
            ) {
                return Err(
                    DefaultExpressionBuildError::UnsupportedLocalFunctionDeclaration(*declaration),
                );
            }
            require_u32_len(captures.len(), DefaultExpressionBuildError::TooManyCaptures)?;
            require_arguments(arguments)?;
        }
        DefaultExpressionKindV1::ContextLookup { .. }
        | DefaultExpressionKindV1::StringLiteral { .. }
        | DefaultExpressionKindV1::IntegerLiteral(_)
        | DefaultExpressionKindV1::CharLiteral(_)
        | DefaultExpressionKindV1::FloatLiteral(_)
        | DefaultExpressionKindV1::BooleanLiteral(_)
        | DefaultExpressionKindV1::UnitLiteral
        | DefaultExpressionKindV1::VariantTest { .. }
        | DefaultExpressionKindV1::VariantPayloadProject { .. }
        | DefaultExpressionKindV1::Local(_)
        | DefaultExpressionKindV1::Capture(_)
        | DefaultExpressionKindV1::GlobalRead(_)
        | DefaultExpressionKindV1::GenericDelegateStorageRead(_)
        | DefaultExpressionKindV1::SingletonValue(_)
        | DefaultExpressionKindV1::Lambda(_)
        | DefaultExpressionKindV1::AnonymousFunction(_)
        | DefaultExpressionKindV1::CallableReference(_)
        | DefaultExpressionKindV1::FunctionCoercion { .. }
        | DefaultExpressionKindV1::PtrFromNonZeroULong(_)
        | DefaultExpressionKindV1::CharCode(_)
        | DefaultExpressionKindV1::CharFromCodeUnchecked(_)
        | DefaultExpressionKindV1::PtrToULong(_)
        | DefaultExpressionKindV1::PtrCast(_)
        | DefaultExpressionKindV1::PtrLoad { .. }
        | DefaultExpressionKindV1::PtrStore { .. }
        | DefaultExpressionKindV1::PtrOffset { .. }
        | DefaultExpressionKindV1::AddressOf(_)
        | DefaultExpressionKindV1::SizeOf(_)
        | DefaultExpressionKindV1::AlignOf(_)
        | DefaultExpressionKindV1::FunctionAddress(_)
        | DefaultExpressionKindV1::ForeignCallbackRegister { .. }
        | DefaultExpressionKindV1::ForeignCallbackOperation { .. }
        | DefaultExpressionKindV1::FieldAccess { .. }
        | DefaultExpressionKindV1::ReleaseFieldLoad { .. }
        | DefaultExpressionKindV1::Box(_)
        | DefaultExpressionKindV1::Unbox(_)
        | DefaultExpressionKindV1::ReferenceUpcast(_)
        | DefaultExpressionKindV1::IsInstance { .. }
        | DefaultExpressionKindV1::Cast { .. }
        | DefaultExpressionKindV1::ArrayGenerate { .. }
        | DefaultExpressionKindV1::ArrayAssembly(_)
        | DefaultExpressionKindV1::Index { .. }
        | DefaultExpressionKindV1::ArraySet { .. }
        | DefaultExpressionKindV1::ArrayLen(_)
        | DefaultExpressionKindV1::ArrayClone(_)
        | DefaultExpressionKindV1::PrimitiveBinary { .. }
        | DefaultExpressionKindV1::PrimitiveUnary { .. }
        | DefaultExpressionKindV1::IntegerOperation { .. }
        | DefaultExpressionKindV1::IntegerConversion { .. }
        | DefaultExpressionKindV1::FloatUnary { .. }
        | DefaultExpressionKindV1::FloatBinary { .. }
        | DefaultExpressionKindV1::FloatConversion { .. }
        | DefaultExpressionKindV1::Binary { .. }
        | DefaultExpressionKindV1::Unary { .. }
        | DefaultExpressionKindV1::SomeWrap(_)
        | DefaultExpressionKindV1::NoneLiteral
        | DefaultExpressionKindV1::IsSome(_)
        | DefaultExpressionKindV1::Unwrap { .. } => {}
    }
    Ok(())
}

fn require_arguments(arguments: &[DefaultExpressionV1]) -> Result<(), DefaultExpressionBuildError> {
    require_u32_len(
        arguments.len(),
        DefaultExpressionBuildError::TooManyArguments,
    )
    .map(|_| ())
}

fn require_u32_len<E>(length: usize, error: E) -> Result<u32, E> {
    u32::try_from(length).map_err(|_| error)
}

#[cfg(test)]
mod tests;
