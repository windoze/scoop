use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeAliasId, SourceContextKey,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{DecodedTypeAliasTargetV1, TypeAliasTargetResolutionError, TypeAliasTargetV1};
use crate::{
    DecodedExportDefinitionSourceV1, ExportDefinitionSourceV1, PublicLookupAccessV1,
    SignatureTypeReferenceResolver,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasInterfaceRecordV1 {
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
    access: PublicLookupAccessV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl TypeAliasInterfaceRecordV1 {
    pub fn try_new(
        alias: PersistentTypeAliasId,
        target: TypeAliasTargetV1,
        access: PublicLookupAccessV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, TypeAliasInterfaceRecordBuildError> {
        if access != PublicLookupAccessV1::DirectOnly {
            return Err(TypeAliasInterfaceRecordBuildError::DirectAccessRequired {
                alias,
                actual: access,
            });
        }
        if target == TypeAliasTargetV1::Alias(alias) {
            return Err(TypeAliasInterfaceRecordBuildError::DirectSelfReference(
                alias,
            ));
        }
        Ok(Self {
            alias,
            target,
            access,
            definition_origin,
        })
    }

    pub const fn alias(&self) -> PersistentTypeAliasId {
        self.alias
    }

    pub const fn target(&self) -> &TypeAliasTargetV1 {
        &self.target
    }

    pub const fn access(&self) -> PublicLookupAccessV1 {
        self.access
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

impl WireEncode for TypeAliasInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.alias.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.access.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeAliasInterfaceRecordV1 {
    alias: DecodedPersistentId<PersistentTypeAliasId>,
    target: DecodedTypeAliasTargetV1,
    access: PublicLookupAccessV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedTypeAliasInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TypeAliasInterfaceRecordV1, TypeAliasInterfaceRecordResolutionError<E>>
    where
        R: TypeAliasInterfaceRecordResolver<E>,
    {
        let alias = resolver
            .resolve(self.alias)
            .map_err(TypeAliasInterfaceRecordResolutionError::Alias)?;
        let target = self
            .target
            .resolve(resolver)
            .map_err(TypeAliasInterfaceRecordResolutionError::Target)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(TypeAliasInterfaceRecordResolutionError::DefinitionOrigin)?;
        TypeAliasInterfaceRecordV1::try_new(alias, target, self.access, definition_origin)
            .map_err(TypeAliasInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedTypeAliasInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.alias.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.access.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedTypeAliasInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            alias: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedTypeAliasTargetV1::decode)?,
            access: decoder.field(3, PublicLookupAccessV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

pub trait TypeAliasInterfaceRecordResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentTypeAliasId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> TypeAliasInterfaceRecordResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentTypeAliasId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAliasInterfaceRecordBuildError {
    DirectAccessRequired {
        alias: PersistentTypeAliasId,
        actual: PublicLookupAccessV1,
    },
    DirectSelfReference(PersistentTypeAliasId),
}

impl fmt::Display for TypeAliasInterfaceRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectAccessRequired { alias, actual } => write!(
                formatter,
                "type alias {alias:?} requires direct-only access, found {actual:?}"
            ),
            Self::DirectSelfReference(alias) => {
                write!(formatter, "type alias {alias:?} directly references itself")
            }
        }
    }
}

impl std::error::Error for TypeAliasInterfaceRecordBuildError {}

#[derive(Debug)]
pub enum TypeAliasInterfaceRecordResolutionError<E> {
    Alias(E),
    Target(TypeAliasTargetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(TypeAliasInterfaceRecordBuildError),
}

impl<E: fmt::Display> fmt::Display for TypeAliasInterfaceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(error) => write!(formatter, "invalid type-alias declaration: {error}"),
            Self::Target(error) => error.fmt(formatter),
            Self::DefinitionOrigin(error) => {
                write!(formatter, "invalid type-alias definition origin: {error}")
            }
            Self::Record(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TypeAliasInterfaceRecordResolutionError<E>
{
}

#[cfg(test)]
mod tests;
