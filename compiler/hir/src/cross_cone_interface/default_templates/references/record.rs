use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedSignatureTypeKey, PersistentIdResolver,
    PersistentKeyResolver, PersistentObjectValueId, PersistentPropertyId,
    PersistentSourceContextId, SignatureTypeKey, SourceContextKey, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExportDefaultCallableTargetV1, ExportDefaultCallableTargetResolutionError,
    ExportDefaultCallableTargetV1,
};
use crate::{
    CallableDeclarationIdResolver, DecodedDefaultConstructorRefV1, DecodedDefaultFieldRefV1,
    DecodedExportDefinitionSourceV1, DefaultConstructorRefResolutionError, DefaultConstructorRefV1,
    DefaultFieldRefResolutionError, DefaultFieldRefV1, ExportDefinitionSourceV1,
};

/// Typed direct binding and definition location in a default template.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportDefaultReferenceV1<T> {
    target: T,
    definition_origin: ExportDefinitionSourceV1,
}

impl<T> ExportDefaultReferenceV1<T> {
    pub const fn new(target: T, definition_origin: ExportDefinitionSourceV1) -> Self {
        Self {
            target,
            definition_origin,
        }
    }

    pub const fn target(&self) -> &T {
        &self.target
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

impl<T: WireEncode> WireEncode for ExportDefaultReferenceV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportDefaultReferenceV1<T> {
    target: T,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl<T> DecodedExportDefaultReferenceV1<T> {
    pub(crate) fn resolve_with<R, E, U>(
        self,
        resolver: &mut R,

        resolve_target: impl FnOnce(
            T,
            &mut R,
        )
            -> Result<U, ExportDefaultReferenceTargetResolutionError<E>>,
    ) -> Result<ExportDefaultReferenceV1<U>, ExportDefaultReferenceResolutionError<E>>
    where
        R: ExportDefaultReferenceResolver<E>,
    {
        let target = resolve_target(self.target, resolver)
            .map_err(ExportDefaultReferenceResolutionError::Target)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(ExportDefaultReferenceResolutionError::DefinitionOrigin)?;
        Ok(ExportDefaultReferenceV1::new(target, definition_origin))
    }
}

impl<T: WireEncode> WireEncode for DecodedExportDefaultReferenceV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.definition_origin.encode(encoder)
    }
}

impl<T: WireDecode> WireDecode for DecodedExportDefaultReferenceV1<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, T::decode)?,
            definition_origin: decoder.field(2, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

pub type ExportDefaultCallableReferenceV1 = ExportDefaultReferenceV1<ExportDefaultCallableTargetV1>;
pub type ExportDefaultConstructorReferenceV1 = ExportDefaultReferenceV1<DefaultConstructorRefV1>;
pub type ExportDefaultTypeReferenceV1 = ExportDefaultReferenceV1<SignatureTypeKey>;
pub type ExportDefaultGlobalReferenceV1 = ExportDefaultReferenceV1<PersistentPropertyId>;
pub type ExportDefaultSingletonReferenceV1 = ExportDefaultReferenceV1<PersistentObjectValueId>;
pub type ExportDefaultFieldReferenceV1 = ExportDefaultReferenceV1<DefaultFieldRefV1>;

pub type DecodedExportDefaultCallableReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedExportDefaultCallableTargetV1>;
pub type DecodedExportDefaultConstructorReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedDefaultConstructorRefV1>;
pub type DecodedExportDefaultTypeReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedSignatureTypeKey>;
pub type DecodedExportDefaultGlobalReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedPersistentId<PersistentPropertyId>>;
pub type DecodedExportDefaultSingletonReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedPersistentId<PersistentObjectValueId>>;
pub type DecodedExportDefaultFieldReferenceV1 =
    DecodedExportDefaultReferenceV1<DecodedDefaultFieldRefV1>;

pub trait ExportDefaultReferenceResolver<E>:
    CallableDeclarationIdResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> ExportDefaultReferenceResolver<E> for R where
    R: CallableDeclarationIdResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceTargetResolutionError<E> {
    Callable(ExportDefaultCallableTargetResolutionError<E>),
    Constructor(DefaultConstructorRefResolutionError<E>),
    Type(E),
    Global(E),
    Singleton(E),
    Field(DefaultFieldRefResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultReferenceTargetResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callable(error) => {
                write!(formatter, "invalid callable reference target: {error}")
            }
            Self::Constructor(error) => {
                write!(formatter, "invalid constructor reference target: {error}")
            }
            Self::Type(error) => write!(formatter, "invalid type reference target: {error}"),
            Self::Global(error) => write!(formatter, "invalid global reference target: {error}"),
            Self::Singleton(error) => {
                write!(formatter, "invalid singleton reference target: {error}")
            }
            Self::Field(error) => write!(formatter, "invalid field reference target: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultReferenceTargetResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceResolutionError<E> {
    Target(ExportDefaultReferenceTargetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultReferenceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "{error}"),
            Self::DefinitionOrigin(error) => {
                write!(formatter, "invalid reference definition origin: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultReferenceResolutionError<E>
{
}
