use std::fmt;

use scoop_identity::{
    DecodedCallableTemplateOrigin, DecodedPersistentId, DecodedSignatureTypeKey,
    PersistentGeneratedCallableId, StructuralDefinitionPath,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::super::{
    DecodedDefaultExpressionV1, DefaultExpressionReferenceResolver,
    DefaultExpressionResolutionError,
};
use super::{
    DefaultCallableReferenceBuildError, DefaultCallableReferenceTargetV1,
    DefaultCallableReferenceV1,
};
use crate::{
    DecodedDefaultCallableRefV1, DecodedDefaultCaptureV1, DecodedDefaultMethodCalleeV1,
    DefaultCallableRefResolutionError, DefaultCaptureResolutionError,
    DefaultMethodCalleeResolutionError, TemplateLocalSelectorResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCallableReferenceV1 {
    invoke: DecodedPersistentId<PersistentGeneratedCallableId>,
    definition_path: StructuralDefinitionPath,
    target: DecodedDefaultCallableReferenceTargetV1,
    function_type: DecodedSignatureTypeKey,
    captures: Vec<DecodedDefaultCaptureV1>,
    owner_type_parameter_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultCallableReferenceTargetV1 {
    Named(DecodedDefaultCallableRefV1),
    Local {
        declaration: DecodedCallableTemplateOrigin,
        callee: DecodedDefaultCallableRefV1,
    },
    BoundMember {
        receiver: Box<DecodedDefaultExpressionV1>,
        callee: DecodedDefaultMethodCalleeV1,
    },
    BoundExtension {
        receiver: Box<DecodedDefaultExpressionV1>,
        callee: DecodedDefaultCallableRefV1,
    },
}

impl DecodedDefaultCallableReferenceV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultCallableReferenceV1, DefaultCallableReferenceResolutionError<E, L::Error>>
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let invoke = resolver
            .resolve(self.invoke)
            .map_err(DefaultCallableReferenceResolutionError::Invoke)?;
        let target = self.target.resolve(resolver, locals)?;
        let function_type = self
            .function_type
            .resolve(resolver)
            .map_err(DefaultCallableReferenceResolutionError::FunctionType)?;
        let capture_count = u32::try_from(self.captures.len()).map_err(|_| {
            DefaultCallableReferenceResolutionError::Record(
                DefaultCallableReferenceBuildError::TooManyCaptures,
            )
        })?;
        let mut captures = Vec::with_capacity(capture_count as usize);
        for (index, capture) in self.captures.into_iter().enumerate() {
            captures.push(capture.resolve(resolver, locals).map_err(|error| {
                DefaultCallableReferenceResolutionError::Capture { index, error }
            })?);
        }
        DefaultCallableReferenceV1::try_new(
            invoke,
            self.definition_path,
            target,
            function_type,
            captures,
            self.owner_type_parameter_count,
        )
        .map_err(DefaultCallableReferenceResolutionError::Record)
    }
}

impl DecodedDefaultCallableReferenceTargetV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<
        DefaultCallableReferenceTargetV1,
        DefaultCallableReferenceResolutionError<E, L::Error>,
    >
    where
        R: DefaultExpressionReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Named(callee) => callee
                .resolve(resolver)
                .map(DefaultCallableReferenceTargetV1::Named)
                .map_err(DefaultCallableReferenceResolutionError::NamedTarget),
            Self::Local {
                declaration,
                callee,
            } => Ok(DefaultCallableReferenceTargetV1::Local {
                declaration: declaration
                    .resolve(resolver)
                    .map_err(DefaultCallableReferenceResolutionError::LocalDeclaration)?,
                callee: callee
                    .resolve(resolver)
                    .map_err(DefaultCallableReferenceResolutionError::LocalCallee)?,
            }),
            Self::BoundMember { receiver, callee } => {
                Ok(DefaultCallableReferenceTargetV1::BoundMember {
                    receiver: Box::new(receiver.resolve(resolver, locals).map_err(|error| {
                        DefaultCallableReferenceResolutionError::TargetReceiver {
                            target_tag: 3,
                            error: Box::new(error),
                        }
                    })?),
                    callee: callee
                        .resolve(resolver)
                        .map_err(DefaultCallableReferenceResolutionError::MemberCallee)?,
                })
            }
            Self::BoundExtension { receiver, callee } => {
                Ok(DefaultCallableReferenceTargetV1::BoundExtension {
                    receiver: Box::new(receiver.resolve(resolver, locals).map_err(|error| {
                        DefaultCallableReferenceResolutionError::TargetReceiver {
                            target_tag: 4,
                            error: Box::new(error),
                        }
                    })?),
                    callee: callee
                        .resolve(resolver)
                        .map_err(DefaultCallableReferenceResolutionError::ExtensionCallee)?,
                })
            }
        }
    }
}

impl WireEncode for DecodedDefaultCallableReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.invoke.encode(encoder)?;
        encoder.field(2)?;
        self.definition_path.encode(encoder)?;
        encoder.field(3)?;
        self.target.encode(encoder)?;
        encoder.field(4)?;
        self.function_type.encode(encoder)?;
        encoder.field(5)?;
        encode_sequence(encoder, &self.captures)?;
        encoder.field(6)?;
        encoder.unsigned(u64::from(self.owner_type_parameter_count))
    }
}

impl WireDecode for DecodedDefaultCallableReferenceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            invoke: decoder.field(1, DecodedPersistentId::decode)?,
            definition_path: decoder.field(2, StructuralDefinitionPath::decode)?,
            target: decoder.field(3, DecodedDefaultCallableReferenceTargetV1::decode)?,
            function_type: decoder.field(4, DecodedSignatureTypeKey::decode)?,
            captures: decoder.field(5, |decoder| {
                decoder.decode_array(|decoder, _| DecodedDefaultCaptureV1::decode(decoder))
            })?,
            owner_type_parameter_count: decoder.field(6, Decoder::u32)?,
        })
    }
}

impl WireEncode for DecodedDefaultCallableReferenceTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named(callee) => encode_one(encoder, 1, callee),
            Self::Local {
                declaration,
                callee,
            } => encode_two(encoder, 2, declaration, callee),
            Self::BoundMember { receiver, callee } => {
                encode_two(encoder, 3, receiver.as_ref(), callee)
            }
            Self::BoundExtension { receiver, callee } => {
                encode_two(encoder, 4, receiver.as_ref(), callee)
            }
        }
    }
}

impl WireDecode for DecodedDefaultCallableReferenceTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultCallableRefV1::decode)
                    .map(Self::Named)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Local {
                    declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
                    callee: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::BoundMember {
                    receiver: decoder
                        .field(1, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                    callee: decoder.field(2, DecodedDefaultMethodCalleeV1::decode)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::BoundExtension {
                    receiver: decoder
                        .field(1, DecodedDefaultExpressionV1::decode)
                        .map(Box::new)?,
                    callee: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCallableReferenceResolutionError<E, L> {
    Invoke(E),
    FunctionType(E),
    NamedTarget(DefaultCallableRefResolutionError<E>),
    LocalDeclaration(E),
    LocalCallee(DefaultCallableRefResolutionError<E>),
    TargetReceiver {
        target_tag: u64,
        error: Box<DefaultExpressionResolutionError<E, L>>,
    },
    MemberCallee(DefaultMethodCalleeResolutionError<E>),
    ExtensionCallee(DefaultCallableRefResolutionError<E>),
    Capture {
        index: usize,
        error: DefaultCaptureResolutionError<E, L>,
    },
    Record(DefaultCallableReferenceBuildError),
}

impl<E: fmt::Display, L: fmt::Display> fmt::Display
    for DefaultCallableReferenceResolutionError<E, L>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invoke(error) => write!(formatter, "invalid callable-reference invoke: {error}"),
            Self::FunctionType(error) => {
                write!(
                    formatter,
                    "invalid callable-reference function type: {error}"
                )
            }
            Self::NamedTarget(error) => {
                write!(
                    formatter,
                    "invalid named callable-reference target: {error}"
                )
            }
            Self::LocalDeclaration(error) => {
                write!(
                    formatter,
                    "invalid local callable-reference declaration: {error}"
                )
            }
            Self::LocalCallee(error) => {
                write!(
                    formatter,
                    "invalid local callable-reference callee: {error}"
                )
            }
            Self::TargetReceiver { target_tag, error } => write!(
                formatter,
                "invalid callable-reference target {target_tag} receiver: {error}"
            ),
            Self::MemberCallee(error) => {
                write!(
                    formatter,
                    "invalid bound-member callable-reference callee: {error}"
                )
            }
            Self::ExtensionCallee(error) => write!(
                formatter,
                "invalid bound-extension callable-reference callee: {error}"
            ),
            Self::Capture { index, error } => {
                write!(
                    formatter,
                    "invalid callable-reference capture {index}: {error}"
                )
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, L: std::error::Error + 'static> std::error::Error
    for DefaultCallableReferenceResolutionError<E, L>
{
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_one(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
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
