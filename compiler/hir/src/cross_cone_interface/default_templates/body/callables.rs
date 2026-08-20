use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedOptionalSignatureType,
    DecodedPersistentId, DecodedSignatureTypeKey, OptionalSignatureType,
    PersistentGeneratedCallableId, PersistentIdResolver, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CallableDeclarationIdResolver, SignatureTypeReferenceResolver};

mod resources;

/// Stable declaration identity accepted by a callable use in a default body.
///
/// Constructors remain a distinct reference domain. Generated callables are
/// admitted here because lexical closure bodies have no source declaration id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultCallableDeclarationV1 {
    Function(scoop_identity::PersistentFunctionId),
    GenericFunction(scoop_identity::PersistentGenericFunctionId),
    PropertyAccessor(scoop_identity::PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl DefaultCallableDeclarationV1 {
    pub const fn template_owner(self) -> scoop_identity::CallableTemplateOwner {
        use scoop_identity::CallableTemplateOwner as Owner;
        match self {
            Self::Function(id) => Owner::Function(id),
            Self::GenericFunction(id) => Owner::GenericFunction(id),
            Self::PropertyAccessor(id) => Owner::Accessor(id),
            Self::Generated(id) => Owner::Generated(id),
        }
    }
}

impl WireEncode for DefaultCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::PropertyAccessor(_) => 3,
            Self::Generated(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(id) => id.encode(encoder),
            Self::GenericFunction(id) => id.encode(encoder),
            Self::PropertyAccessor(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDefaultCallableDeclarationV1 {
    Function(DecodedPersistentId<scoop_identity::PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<scoop_identity::PersistentGenericFunctionId>),
    PropertyAccessor(DecodedPersistentId<scoop_identity::PersistentPropertyAccessorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedDefaultCallableDeclarationV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefaultCallableDeclarationV1, E>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::GenericFunction),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::PropertyAccessor),
            Self::Generated(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::Generated),
        }
    }
}

impl WireEncode for DecodedDefaultCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::PropertyAccessor(_) => 3,
            Self::Generated(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(id) => id.encode(encoder),
            Self::GenericFunction(id) => id.encode(encoder),
            Self::PropertyAccessor(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultCallableDeclarationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

/// One declaration-bound callable application in a default template.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCallableRefV1 {
    declaration: DefaultCallableDeclarationV1,
    owner: OptionalSignatureType,
    type_arguments: Vec<SignatureTypeKey>,
}

impl DefaultCallableRefV1 {
    pub fn try_new(
        declaration: DefaultCallableDeclarationV1,
        owner: OptionalSignatureType,
        type_arguments: Vec<SignatureTypeKey>,
    ) -> Result<Self, DefaultCallableRefBuildError> {
        u32::try_from(type_arguments.len())
            .map_err(|_| DefaultCallableRefBuildError::TooManyTypeArguments)?;
        Ok(Self {
            declaration,
            owner,
            type_arguments,
        })
    }

    pub const fn declaration(&self) -> DefaultCallableDeclarationV1 {
        self.declaration
    }

    pub const fn owner(&self) -> &OptionalSignatureType {
        &self.owner
    }

    pub fn type_arguments(&self) -> &[SignatureTypeKey] {
        &self.type_arguments
    }
}

impl WireEncode for DefaultCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.type_arguments)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCallableRefV1 {
    declaration: DecodedDefaultCallableDeclarationV1,
    owner: DecodedOptionalSignatureType,
    type_arguments: Vec<DecodedSignatureTypeKey>,
}

impl DecodedDefaultCallableRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultCallableRefV1, DefaultCallableRefResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(DefaultCallableRefResolutionError::Declaration)?;
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(DefaultCallableRefResolutionError::Owner)?;
        let count = u32::try_from(self.type_arguments.len())
            .map_err(|_| DefaultCallableRefResolutionError::TooManyTypeArguments)?;
        let mut type_arguments = Vec::with_capacity(count as usize);
        for (index, argument) in self.type_arguments.into_iter().enumerate() {
            type_arguments.push(argument.resolve(resolver).map_err(|error| {
                DefaultCallableRefResolutionError::TypeArgument { index, error }
            })?);
        }
        Ok(DefaultCallableRefV1 {
            declaration,
            owner,
            type_arguments,
        })
    }
}

impl WireEncode for DecodedDefaultCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.type_arguments)
    }
}

impl WireDecode for DecodedDefaultCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedDefaultCallableDeclarationV1::decode)?,
            owner: decoder.field(2, DecodedOptionalSignatureType::decode)?,
            type_arguments: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultBoundCallableSourceV1 {
    Class {
        bound: SignatureTypeKey,
        callable: DefaultCallableRefV1,
    },
    Interface {
        bound: SignatureTypeKey,
        member: CallableTemplateOrigin,
    },
}

impl WireEncode for DefaultBoundCallableSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Class { .. } => 1,
            Self::Interface { .. } => 2,
        })?;
        match self {
            Self::Class { bound, callable } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                callable.encode(encoder)
            }
            Self::Interface { bound, member } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                member.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultBoundCallableSourceV1 {
    Class {
        bound: DecodedSignatureTypeKey,
        callable: DecodedDefaultCallableRefV1,
    },
    Interface {
        bound: DecodedSignatureTypeKey,
        member: DecodedCallableTemplateOrigin,
    },
}

impl DecodedDefaultBoundCallableSourceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultBoundCallableSourceV1, DefaultBoundCallableSourceResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Class { bound, callable } => Ok(DefaultBoundCallableSourceV1::Class {
                bound: bound
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Bound)?,
                callable: callable
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Callable)?,
            }),
            Self::Interface { bound, member } => Ok(DefaultBoundCallableSourceV1::Interface {
                bound: bound
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Bound)?,
                member: member
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Member)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultBoundCallableSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Class { .. } => 1,
            Self::Interface { .. } => 2,
        })?;
        match self {
            Self::Class { bound, callable } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                callable.encode(encoder)
            }
            Self::Interface { bound, member } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                member.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDefaultBoundCallableSourceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::Class {
                bound: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                callable: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
            }),
            2 => Ok(Self::Interface {
                bound: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                member: decoder.field(2, DecodedCallableTemplateOrigin::decode)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBoundCallableRefV1 {
    receiver_type: SignatureTypeKey,
    source: DefaultBoundCallableSourceV1,
    instantiated_signature: SignatureTypeKey,
}

impl DefaultBoundCallableRefV1 {
    pub const fn new(
        receiver_type: SignatureTypeKey,
        source: DefaultBoundCallableSourceV1,
        instantiated_signature: SignatureTypeKey,
    ) -> Self {
        Self {
            receiver_type,
            source,
            instantiated_signature,
        }
    }

    pub const fn receiver_type(&self) -> &SignatureTypeKey {
        &self.receiver_type
    }

    pub const fn source(&self) -> &DefaultBoundCallableSourceV1 {
        &self.source
    }

    pub const fn instantiated_signature(&self) -> &SignatureTypeKey {
        &self.instantiated_signature
    }
}

impl WireEncode for DefaultBoundCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver_type.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.instantiated_signature.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBoundCallableRefV1 {
    receiver_type: DecodedSignatureTypeKey,
    source: DecodedDefaultBoundCallableSourceV1,
    instantiated_signature: DecodedSignatureTypeKey,
}

impl DecodedDefaultBoundCallableRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultBoundCallableRefV1, DefaultBoundCallableRefResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        Ok(DefaultBoundCallableRefV1 {
            receiver_type: self
                .receiver_type
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::ReceiverType)?,
            source: self
                .source
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::Source)?,
            instantiated_signature: self
                .instantiated_signature
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::InstantiatedSignature)?,
        })
    }
}

impl WireEncode for DecodedDefaultBoundCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver_type.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.instantiated_signature.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultBoundCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            receiver_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            source: decoder.field(2, DecodedDefaultBoundCallableSourceV1::decode)?,
            instantiated_signature: decoder.field(3, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultMethodCalleeV1 {
    Callable(DefaultCallableRefV1),
    Bound(DefaultBoundCallableRefV1),
    DerivedEquality { owner_type: SignatureTypeKey },
}

impl WireEncode for DefaultMethodCalleeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(callable) => callable.encode(encoder),
            Self::Bound(bound) => bound.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultMethodCalleeV1 {
    Callable(DecodedDefaultCallableRefV1),
    Bound(DecodedDefaultBoundCallableRefV1),
    DerivedEquality { owner_type: DecodedSignatureTypeKey },
}

impl DecodedDefaultMethodCalleeV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultMethodCalleeV1, DefaultMethodCalleeResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Callable(callable) => callable
                .resolve(resolver)
                .map(DefaultMethodCalleeV1::Callable)
                .map_err(DefaultMethodCalleeResolutionError::Callable),
            Self::Bound(bound) => bound
                .resolve(resolver)
                .map(DefaultMethodCalleeV1::Bound)
                .map_err(DefaultMethodCalleeResolutionError::Bound),
            Self::DerivedEquality { owner_type } => owner_type
                .resolve(resolver)
                .map(|owner_type| DefaultMethodCalleeV1::DerivedEquality { owner_type })
                .map_err(DefaultMethodCalleeResolutionError::DerivedEqualityOwner),
        }
    }
}

impl WireEncode for DecodedDefaultMethodCalleeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(callable) => callable.encode(encoder),
            Self::Bound(bound) => bound.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultMethodCalleeV1 {
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
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub trait DefaultCallableReferenceResolver<E>:
    SignatureTypeReferenceResolver<E>
    + CallableDeclarationIdResolver<E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

impl<R, E> DefaultCallableReferenceResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + CallableDeclarationIdResolver<E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultCallableRefBuildError {
    TooManyTypeArguments,
}

impl fmt::Display for DefaultCallableRefBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyTypeArguments => {
                formatter.write_str("default callable type argument count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultCallableRefBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCallableRefResolutionError<E> {
    Declaration(E),
    Owner(E),
    TooManyTypeArguments,
    TypeArgument { index: usize, error: E },
}

impl<E: fmt::Display> fmt::Display for DefaultCallableRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => {
                write!(formatter, "invalid default callable declaration: {error}")
            }
            Self::Owner(error) => write!(formatter, "invalid default callable owner: {error}"),
            Self::TooManyTypeArguments => {
                formatter.write_str("default callable type argument count exceeds u32")
            }
            Self::TypeArgument { index, error } => {
                write!(
                    formatter,
                    "invalid default callable type argument {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultCallableRefResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBoundCallableSourceResolutionError<E> {
    Bound(E),
    Callable(DefaultCallableRefResolutionError<E>),
    Member(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBoundCallableSourceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(error) => write!(formatter, "invalid bound callable constraint: {error}"),
            Self::Callable(error) => write!(formatter, "invalid bound class callable: {error}"),
            Self::Member(error) => write!(formatter, "invalid bound interface member: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBoundCallableSourceResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBoundCallableRefResolutionError<E> {
    ReceiverType(E),
    Source(DefaultBoundCallableSourceResolutionError<E>),
    InstantiatedSignature(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBoundCallableRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReceiverType(error) => {
                write!(formatter, "invalid bound callable receiver type: {error}")
            }
            Self::Source(error) => write!(formatter, "invalid bound callable source: {error}"),
            Self::InstantiatedSignature(error) => {
                write!(formatter, "invalid bound callable signature: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBoundCallableRefResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultMethodCalleeResolutionError<E> {
    Callable(DefaultCallableRefResolutionError<E>),
    Bound(DefaultBoundCallableRefResolutionError<E>),
    DerivedEqualityOwner(E),
}

impl<E: fmt::Display> fmt::Display for DefaultMethodCalleeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callable(error) => write!(formatter, "invalid method callable: {error}"),
            Self::Bound(error) => write!(formatter, "invalid bound method callable: {error}"),
            Self::DerivedEqualityOwner(error) => {
                write!(formatter, "invalid derived equality owner: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultMethodCalleeResolutionError<E> {}

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

#[cfg(test)]
mod tests;
