use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedSignatureTypeKey, PersistentIdResolver,
    PersistentKeyResolver, PersistentPropertyId, PersistentSourceContextId, SignatureTypeKey,
    SourceContextKey, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::CanonicalConstValueV1;
use crate::{
    DecodedExportDefinitionSourceV1, ExportDefinitionSourceV1, SignatureTypeReferenceResolver,
};

mod semantics;

pub use semantics::{
    ConstPropertyDeclarationSourceV1, ExportConstValueSemanticAuthority,
    ExportConstValueSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportConstValueV1 {
    property: PersistentPropertyId,
    value_type: SignatureTypeKey,
    value: CanonicalConstValueV1,
    definition_origin: ExportDefinitionSourceV1,
}

impl ExportConstValueV1 {
    pub fn new(
        property: PersistentPropertyId,
        value_type: SignatureTypeKey,
        value: CanonicalConstValueV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            property,
            value_type,
            value,
            definition_origin,
        }
    }

    pub const fn property(&self) -> PersistentPropertyId {
        self.property
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn value(&self) -> &CanonicalConstValueV1 {
        &self.value
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

impl WireEncode for ExportConstValueV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.property.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.value.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportConstValueV1 {
    property: DecodedPersistentId<PersistentPropertyId>,
    value_type: DecodedSignatureTypeKey,
    value: CanonicalConstValueV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedExportConstValueV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportConstValueV1, ExportConstValueResolutionError<E>>
    where
        R: ExportConstValueResolver<E>,
    {
        let property = resolver
            .resolve(self.property)
            .map_err(ExportConstValueResolutionError::Property)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(ExportConstValueResolutionError::ValueType)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(ExportConstValueResolutionError::DefinitionOrigin)?;
        Ok(ExportConstValueV1::new(
            property,
            value_type,
            self.value,
            definition_origin,
        ))
    }
}

impl WireEncode for DecodedExportConstValueV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.property.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.value.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedExportConstValueV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            property: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            value: decoder.field(3, CanonicalConstValueV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

pub trait ExportConstValueResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> ExportConstValueResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Debug)]
pub enum ExportConstValueResolutionError<E> {
    Property(E),
    ValueType(E),
    DefinitionOrigin(SourceOriginResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for ExportConstValueResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Property(error) => write!(formatter, "invalid const property: {error}"),
            Self::ValueType(error) => write!(formatter, "invalid const value type: {error}"),
            Self::DefinitionOrigin(error) => {
                write!(formatter, "invalid const definition origin: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExportConstValueResolutionError<E> {}

#[cfg(test)]
mod tests;
