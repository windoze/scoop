use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentIdResolver, PersistentKeyResolver, PersistentSourceContextId,
    SourceContextKey, SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::wire;
use crate::{
    DecodedExportDefinitionSourceV1, DecodedSourceNominalId, ExportDefinitionSourceV1,
    SourceNominalId, SourceNominalIdResolver,
};

/// A closed wire enum, independent of source syntax omission or numeric
/// ordering between visibility levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredVisibilityV1 {
    Public,
    Internal,
    Private,
    Protected,
}

impl From<crate::DeclaredVisibility> for DeclaredVisibilityV1 {
    fn from(value: crate::DeclaredVisibility) -> Self {
        match value {
            crate::DeclaredVisibility::Public => Self::Public,
            crate::DeclaredVisibility::Internal => Self::Internal,
            crate::DeclaredVisibility::Private => Self::Private,
            crate::DeclaredVisibility::Protected => Self::Protected,
        }
    }
}

impl WireEncode for DeclaredVisibilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::tag(
            encoder,
            1,
            match self {
                Self::Public => 1,
                Self::Internal => 2,
                Self::Private => 3,
                Self::Protected => 4,
            },
        )
    }
}

impl WireDecode for DeclaredVisibilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Public),
            2 => Ok(Self::Internal),
            3 => Ok(Self::Private),
            4 => Ok(Self::Protected),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationAccessSourceV1 {
    declared_visibility: DeclaredVisibilityV1,
    lexical_owners: Vec<SourceNominalId>,
    definition_origin: ExportDefinitionSourceV1,
}

impl DeclarationAccessSourceV1 {
    pub fn try_new(
        declared_visibility: DeclaredVisibilityV1,
        lexical_owners: Vec<SourceNominalId>,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, DeclarationAccessSourceBuildError> {
        let mut seen = BTreeSet::new();
        for (index, owner) in lexical_owners.iter().enumerate() {
            if !seen.insert(*owner) {
                return Err(DeclarationAccessSourceBuildError::RepeatedOwner { index });
            }
        }
        if declared_visibility == DeclaredVisibilityV1::Protected && lexical_owners.is_empty() {
            return Err(DeclarationAccessSourceBuildError::TopLevelProtected);
        }
        Ok(Self {
            declared_visibility,
            lexical_owners,
            definition_origin,
        })
    }
    pub const fn declared_visibility(&self) -> DeclaredVisibilityV1 {
        self.declared_visibility
    }
    pub fn lexical_owners(&self) -> &[SourceNominalId] {
        &self.lexical_owners
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

impl WireEncode for DeclarationAccessSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declared_visibility.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.lexical_owners)?;
        encoder.field(3)?;
        self.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDeclarationAccessSourceV1 {
    declared_visibility: DeclaredVisibilityV1,
    lexical_owners: Vec<DecodedSourceNominalId>,
    definition_origin: DecodedExportDefinitionSourceV1,
}

impl DecodedDeclarationAccessSourceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DeclarationAccessSourceV1, DeclarationAccessSourceResolutionError<E>>
    where
        R: SourceNominalIdResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let owners = self
            .lexical_owners
            .into_iter()
            .enumerate()
            .map(|(index, owner)| {
                owner
                    .resolve(resolver)
                    .map_err(|error| DeclarationAccessSourceResolutionError::Owner { index, error })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(DeclarationAccessSourceResolutionError::Origin)?;
        DeclarationAccessSourceV1::try_new(self.declared_visibility, owners, origin)
            .map_err(DeclarationAccessSourceResolutionError::Source)
    }
}

impl WireEncode for DecodedDeclarationAccessSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declared_visibility.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.lexical_owners)?;
        encoder.field(3)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDeclarationAccessSourceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            declared_visibility: decoder.field(1, DeclaredVisibilityV1::decode)?,
            lexical_owners: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedSourceNominalId::decode(decoder))
            })?,
            definition_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationAccessSourceBuildError {
    RepeatedOwner { index: usize },
    TopLevelProtected,
}
impl fmt::Display for DeclarationAccessSourceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepeatedOwner { index } => write!(f, "repeated lexical owner at index {index}"),
            Self::TopLevelProtected => {
                f.write_str("protected declarations require a lexical class owner")
            }
        }
    }
}
impl std::error::Error for DeclarationAccessSourceBuildError {}

#[derive(Debug)]
pub enum DeclarationAccessSourceResolutionError<E> {
    Resource(WireError),
    Owner { index: usize, error: E },
    Origin(SourceOriginResolutionError<E>),
    Source(DeclarationAccessSourceBuildError),
}
impl<E: fmt::Display> fmt::Display for DeclarationAccessSourceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Owner { index, error } => {
                write!(f, "invalid lexical owner at index {index}: {error}")
            }
            Self::Origin(error) => write!(f, "invalid access definition source: {error}"),
            Self::Source(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for DeclarationAccessSourceResolutionError<E>
{
}
