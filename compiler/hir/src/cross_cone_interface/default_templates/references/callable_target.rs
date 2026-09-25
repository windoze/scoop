use std::fmt;

mod resources;

use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedPersistentId,
    DecodedSignatureTypeKey, PersistentGeneratedCallableId, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedDefaultBoundCallableRefV1, DecodedDefaultCallableDeclarationV1,
    DecodedDefaultCallableRefV1, DefaultBoundCallableRefResolutionError, DefaultBoundCallableRefV1,
    DefaultCallableDeclarationV1, DefaultCallableRefResolutionError, DefaultCallableRefV1,
    DefaultCallableReferenceResolver,
};

/// One callable target for which a default template carries an access proof.
///
/// The variants mirror the definition-side HIR target categories instead of
/// collapsing lexical wrappers or open derived equality into a source
/// callable declaration that may not exist.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExportDefaultCallableTargetV1 {
    Callable(DefaultCallableRefV1),
    Bound(DefaultBoundCallableRefV1),
    DerivedEquality {
        owner_type: SignatureTypeKey,
    },
    LocalFunction {
        declaration: CallableTemplateOrigin,
    },
    Lambda {
        body: PersistentGeneratedCallableId,
    },
    AnonymousFunction {
        body: PersistentGeneratedCallableId,
    },
    CallableReference {
        invoke: PersistentGeneratedCallableId,
    },
    FunctionAddress {
        declaration: DefaultCallableDeclarationV1,
    },
}

impl ExportDefaultCallableTargetV1 {
    pub(crate) fn validate(&self) -> Result<(), ExportDefaultCallableTargetBuildError> {
        if let Self::LocalFunction { declaration } = self
            && !matches!(
                declaration,
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
            )
        {
            return Err(
                ExportDefaultCallableTargetBuildError::UnsupportedLocalFunctionDeclaration(
                    *declaration,
                ),
            );
        }
        Ok(())
    }
}

impl WireEncode for ExportDefaultCallableTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
            Self::LocalFunction { .. } => 4,
            Self::Lambda { .. } => 5,
            Self::AnonymousFunction { .. } => 6,
            Self::CallableReference { .. } => 7,
            Self::FunctionAddress { .. } => 8,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(target) => target.encode(encoder),
            Self::Bound(target) => target.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
            Self::LocalFunction { declaration } => declaration.encode(encoder),
            Self::Lambda { body } | Self::AnonymousFunction { body } => body.encode(encoder),
            Self::CallableReference { invoke } => invoke.encode(encoder),
            Self::FunctionAddress { declaration } => declaration.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedExportDefaultCallableTargetV1 {
    Callable(DecodedDefaultCallableRefV1),
    Bound(DecodedDefaultBoundCallableRefV1),
    DerivedEquality {
        owner_type: DecodedSignatureTypeKey,
    },
    LocalFunction {
        declaration: DecodedCallableTemplateOrigin,
    },
    Lambda {
        body: DecodedPersistentId<PersistentGeneratedCallableId>,
    },
    AnonymousFunction {
        body: DecodedPersistentId<PersistentGeneratedCallableId>,
    },
    CallableReference {
        invoke: DecodedPersistentId<PersistentGeneratedCallableId>,
    },
    FunctionAddress {
        declaration: DecodedDefaultCallableDeclarationV1,
    },
}

impl DecodedExportDefaultCallableTargetV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportDefaultCallableTargetV1, ExportDefaultCallableTargetResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        let target = match self {
            Self::Callable(target) => target
                .resolve(resolver)
                .map(ExportDefaultCallableTargetV1::Callable)
                .map_err(ExportDefaultCallableTargetResolutionError::Callable)?,
            Self::Bound(target) => target
                .resolve(resolver)
                .map(ExportDefaultCallableTargetV1::Bound)
                .map_err(ExportDefaultCallableTargetResolutionError::Bound)?,
            Self::DerivedEquality { owner_type } => {
                ExportDefaultCallableTargetV1::DerivedEquality {
                    owner_type: owner_type.resolve(resolver).map_err(
                        ExportDefaultCallableTargetResolutionError::DerivedEqualityOwner,
                    )?,
                }
            }
            Self::LocalFunction { declaration } => ExportDefaultCallableTargetV1::LocalFunction {
                declaration: declaration
                    .resolve(resolver)
                    .map_err(ExportDefaultCallableTargetResolutionError::LocalFunction)?,
            },
            Self::Lambda { body } => ExportDefaultCallableTargetV1::Lambda {
                body: resolver
                    .resolve(body)
                    .map_err(ExportDefaultCallableTargetResolutionError::Lambda)?,
            },
            Self::AnonymousFunction { body } => ExportDefaultCallableTargetV1::AnonymousFunction {
                body: resolver
                    .resolve(body)
                    .map_err(ExportDefaultCallableTargetResolutionError::AnonymousFunction)?,
            },
            Self::CallableReference { invoke } => {
                ExportDefaultCallableTargetV1::CallableReference {
                    invoke: resolver
                        .resolve(invoke)
                        .map_err(ExportDefaultCallableTargetResolutionError::CallableReference)?,
                }
            }
            Self::FunctionAddress { declaration } => {
                ExportDefaultCallableTargetV1::FunctionAddress {
                    declaration: declaration
                        .resolve(resolver)
                        .map_err(ExportDefaultCallableTargetResolutionError::FunctionAddress)?,
                }
            }
        };
        target
            .validate()
            .map_err(ExportDefaultCallableTargetResolutionError::Shape)?;
        Ok(target)
    }
}

impl WireEncode for DecodedExportDefaultCallableTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
            Self::LocalFunction { .. } => 4,
            Self::Lambda { .. } => 5,
            Self::AnonymousFunction { .. } => 6,
            Self::CallableReference { .. } => 7,
            Self::FunctionAddress { .. } => 8,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(target) => target.encode(encoder),
            Self::Bound(target) => target.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
            Self::LocalFunction { declaration } => declaration.encode(encoder),
            Self::Lambda { body } | Self::AnonymousFunction { body } => body.encode(encoder),
            Self::CallableReference { invoke } => invoke.encode(encoder),
            Self::FunctionAddress { declaration } => declaration.encode(encoder),
        }
    }
}

impl WireDecode for DecodedExportDefaultCallableTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedDefaultCallableRefV1::decode)
                .map(Self::Callable),
            2 => decoder
                .field(1, DecodedDefaultBoundCallableRefV1::decode)
                .map(Self::Bound),
            3 => decoder
                .field(1, DecodedSignatureTypeKey::decode)
                .map(|owner_type| Self::DerivedEquality { owner_type }),
            4 => decoder
                .field(1, DecodedCallableTemplateOrigin::decode)
                .map(|declaration| Self::LocalFunction { declaration }),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|body| Self::Lambda { body }),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|body| Self::AnonymousFunction { body }),
            7 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|invoke| Self::CallableReference { invoke }),
            8 => decoder
                .field(1, DecodedDefaultCallableDeclarationV1::decode)
                .map(|declaration| Self::FunctionAddress { declaration }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultCallableTargetBuildError {
    UnsupportedLocalFunctionDeclaration(CallableTemplateOrigin),
}

impl fmt::Display for ExportDefaultCallableTargetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLocalFunctionDeclaration(declaration) => write!(
                formatter,
                "default reference local function uses unsupported declaration {declaration:?}"
            ),
        }
    }
}

impl std::error::Error for ExportDefaultCallableTargetBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultCallableTargetResolutionError<E> {
    Callable(DefaultCallableRefResolutionError<E>),
    Bound(DefaultBoundCallableRefResolutionError<E>),
    DerivedEqualityOwner(E),
    LocalFunction(E),
    Lambda(E),
    AnonymousFunction(E),
    CallableReference(E),
    FunctionAddress(E),
    Shape(ExportDefaultCallableTargetBuildError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultCallableTargetResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callable(error) => write!(formatter, "invalid callable target: {error}"),
            Self::Bound(error) => write!(formatter, "invalid bound callable target: {error}"),
            Self::DerivedEqualityOwner(error) => {
                write!(formatter, "invalid derived-equality owner: {error}")
            }
            Self::LocalFunction(error) => write!(formatter, "invalid local function: {error}"),
            Self::Lambda(error) => write!(formatter, "invalid lambda body: {error}"),
            Self::AnonymousFunction(error) => {
                write!(formatter, "invalid anonymous-function body: {error}")
            }
            Self::CallableReference(error) => {
                write!(formatter, "invalid callable-reference invoke: {error}")
            }
            Self::FunctionAddress(error) => {
                write!(formatter, "invalid function-address declaration: {error}")
            }
            Self::Shape(error) => write!(formatter, "invalid callable target shape: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultCallableTargetResolutionError<E>
{
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
