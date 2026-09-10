//! Persistent identities aligned with the export HIR type-alias arena.

use std::fmt;
use std::ops::Index;

use la_arena::Arena;
use scoop_identity::{
    CborIdentityRecord, PersistentTypeAliasId, SourceDeclarationIdentityError, SourceDeclarationKey,
};

use crate::{ExportTypeAliasId, TypeAliasDecl};

#[derive(Clone, Debug)]
pub struct HirTypeAliasIdentities {
    identities: Vec<CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>>,
}

impl HirTypeAliasIdentities {
    pub fn checked(
        aliases: &Arena<TypeAliasDecl>,
        identities: Vec<CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>>,
    ) -> Result<Self, HirTypeAliasIdentityTableError> {
        if aliases.len() == identities.len() {
            Ok(Self { identities })
        } else {
            Err(HirTypeAliasIdentityTableError::Length {
                expected: aliases.len(),
                actual: identities.len(),
            })
        }
    }

    pub fn from_declarations(
        aliases: &Arena<TypeAliasDecl>,
        declarations: Vec<SourceDeclarationKey>,
    ) -> Result<Self, HirTypeAliasIdentityError> {
        let identities = declarations
            .into_iter()
            .map(CborIdentityRecord::from_key)
            .collect::<Result<Vec<_>, _>>()
            .map_err(HirTypeAliasIdentityError::Identity)?;
        Self::checked(aliases, identities).map_err(HirTypeAliasIdentityError::Table)
    }
}

impl Index<ExportTypeAliasId> for HirTypeAliasIdentities {
    type Output = CborIdentityRecord<PersistentTypeAliasId, SourceDeclarationKey>;

    fn index(&self, id: ExportTypeAliasId) -> &Self::Output {
        &self.identities[id.into_raw().into_u32() as usize]
    }
}

#[derive(Debug)]
pub enum HirTypeAliasIdentityError {
    Identity(SourceDeclarationIdentityError),
    Table(HirTypeAliasIdentityTableError),
}

impl fmt::Display for HirTypeAliasIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Table(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HirTypeAliasIdentityError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirTypeAliasIdentityTableError {
    Length { expected: usize, actual: usize },
}

impl fmt::Display for HirTypeAliasIdentityTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => write!(
                formatter,
                "type-alias identity table has {actual} entries, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for HirTypeAliasIdentityTableError {}

#[cfg(test)]
mod tests;
