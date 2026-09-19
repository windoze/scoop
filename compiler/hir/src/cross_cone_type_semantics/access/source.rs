use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ConeIdentity, DefinitionOwnerAtom, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, SourceContextKey, SourceDeclarationKey, SourceDeclarationKind,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::wire;
use crate::{
    DecodedExportDefinitionSourceV1, DecodedSourceNominalId,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceV1, SourceNominalId, SourceNominalIdResolver,
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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

    pub fn validate_for_declaration<'a, A, E>(
        &'a self,
        declaration: &'a SourceDeclarationKey,
        authority: &mut A,
    ) -> Result<CheckedDeclarationAccessSourceV1<'a>, DeclarationAccessSourceSemanticError<E>>
    where
        A: DeclarationAccessSourceSemanticAuthority<E>,
    {
        self.definition_origin
            .validate_semantics(authority)
            .map_err(DeclarationAccessSourceSemanticError::Origin)?;
        let source = self.definition_origin.origin().source();
        if declaration.origin() != source.cone()
            || declaration
                .scope()
                .source()
                .is_some_and(|expected| expected != source)
        {
            return Err(DeclarationAccessSourceSemanticError::DefinitionSource);
        }
        if !owners_match(declaration, &self.lexical_owners) {
            return Err(DeclarationAccessSourceSemanticError::OwnerChain);
        }
        for (index, owner) in self.lexical_owners.iter().enumerate() {
            let key = authority
                .nominal_declaration_key(*owner)
                .map_err(DeclarationAccessSourceSemanticError::Foundation)?;
            if SourceNominalId::from_source_declaration(key).ok() != Some(*owner)
                || !owners_match(key, &self.lexical_owners[..index])
            {
                return Err(DeclarationAccessSourceSemanticError::OwnerKey { index });
            }
            if self.declared_visibility == DeclaredVisibilityV1::Protected
                && index + 1 == self.lexical_owners.len()
                && key.declaration_kind() != SourceDeclarationKind::Class
            {
                return Err(DeclarationAccessSourceSemanticError::ProtectedOwnerNotClass);
            }
            let owner_origin = authority
                .nominal_definition_source(*owner)
                .map_err(DeclarationAccessSourceSemanticError::Foundation)?;
            if owner_origin.origin().source() != source {
                return Err(DeclarationAccessSourceSemanticError::OwnerSource { index });
            }
        }
        Ok(CheckedDeclarationAccessSourceV1 {
            source: self,
            declaration,
        })
    }
}

fn owners_match(key: &SourceDeclarationKey, owners: &[SourceNominalId]) -> bool {
    key.owners().owners().len() == owners.len()
        && key
            .owners()
            .owners()
            .iter()
            .zip(owners)
            .all(|(atom, owner)| match (atom, owner) {
                (DefinitionOwnerAtom::Type(left), SourceNominalId::Concrete(right)) => {
                    left == right
                }
                (
                    DefinitionOwnerAtom::GenericType(left),
                    SourceNominalId::GenericTemplate(right),
                ) => left == right,
                _ => false,
            })
}

pub trait DeclarationAccessSourceSemanticAuthority<E>:
    ExportDefinitionSourceSemanticAuthority<E>
{
    fn nominal_declaration_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, E>;
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, E>;
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedDeclarationAccessSourceV1<'a> {
    source: &'a DeclarationAccessSourceV1,
    declaration: &'a SourceDeclarationKey,
}
impl CheckedDeclarationAccessSourceV1<'_> {
    pub const fn source(&self) -> &DeclarationAccessSourceV1 {
        self.source
    }
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        self.declaration
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    Owner { index: usize, error: E },
    Origin(SourceOriginResolutionError<E>),
    Source(DeclarationAccessSourceBuildError),
}
impl<E: fmt::Display> fmt::Display for DeclarationAccessSourceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

#[derive(Debug, Eq, PartialEq)]
pub enum DeclarationAccessSourceSemanticError<E> {
    Foundation(E),
    Origin(ExportDefinitionSourceSemanticValidationError<E>),
    DefinitionSource,
    OwnerChain,
    OwnerKey { index: usize },
    OwnerSource { index: usize },
    ProtectedOwnerNotClass,
}
impl<E: fmt::Display> fmt::Display for DeclarationAccessSourceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => write!(f, "invalid access source foundation: {error}"),
            Self::Origin(error) => error.fmt(f),
            Self::DefinitionSource => f.write_str(
                "access source disagrees with the declaration's defining Cone or source",
            ),
            Self::OwnerChain => f.write_str(
                "access lexical owner chain disagrees with the canonical declaration key",
            ),
            Self::OwnerKey { index } => {
                write!(f, "access lexical owner key disagrees at index {index}")
            }
            Self::OwnerSource { index } => write!(
                f,
                "access lexical owner definition source disagrees at index {index}"
            ),
            Self::ProtectedOwnerNotClass => {
                f.write_str("protected declaration owner is not a class")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DeclarationAccessSourceSemanticError<E> {}
