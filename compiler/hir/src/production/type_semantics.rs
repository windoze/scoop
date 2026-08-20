//! Projection of complete HIR into the shared type section.

use std::fmt;

use scoop_identity::PersistentExactTypeId;

use crate::{CrossConeTypeSemanticsSectionV1, DependencyHirOutput, SourceNominalId};

mod facts;
pub(in crate::production) mod inheritance;
mod nominals;

impl CrossConeTypeSemanticsSectionV1 {
    /// Projects complete type representations, inheritance and dependency uses
    /// from the same Export/LocalConcrete pair as the public HIR interface.
    /// Source and materialized nominal types use the same projection.
    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
        metadata: crate::SharedTypeMetadataV1<'_>,
        dependencies: &[crate::SharedTypeMetadataV1<'_>],
    ) -> Result<Self, CrossConeTypeSemanticsProductionError> {
        nominals::produce(output, metadata, dependencies)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeSemanticsNominalKind {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
}

#[derive(Debug)]
pub enum CrossConeTypeSemanticsProductionError {
    SourceInventory(crate::SourceInventoryError),
    SharedTypeMetadata(Box<crate::SharedTypeMetadataError>),
    PublicInterface(String),
    MissingNominalIdentity {
        kind: TypeSemanticsNominalKind,
        index: u32,
    },
    MissingDefinitionOrigin(scoop_identity::DefinitionOriginSubject),
    InvalidLexicalOwner(scoop_identity::DefinitionOwnerAtom),
    InvalidSourceDeclaration(String),
    MissingExactIdentity {
        context: &'static str,
    },
    ExactIdentityMismatch(PersistentExactTypeId),
    MissingConcreteType(PersistentExactTypeId),
    MissingLocalSupport(PersistentExactTypeId),
    InvalidSourceShape {
        declaration: SourceNominalId,
        reason: String,
    },
    InvalidFact {
        exact: PersistentExactTypeId,
        reason: String,
    },
    InvalidRepresentation {
        declaration: SourceNominalId,
        reason: String,
    },
    InvalidInheritance {
        exact: PersistentExactTypeId,
        reason: String,
    },
    InvalidTable {
        table: &'static str,
        reason: String,
    },
}

impl fmt::Display for CrossConeTypeSemanticsProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceInventory(error) => error.fmt(f),
            Self::SharedTypeMetadata(error) => error.fmt(f),
            Self::PublicInterface(reason) => {
                write!(f, "cannot project the public interface: {reason}")
            }
            Self::MissingNominalIdentity { kind, index } => {
                write!(f, "missing {kind:?} identity at arena index {index}")
            }
            Self::MissingDefinitionOrigin(subject) => {
                write!(f, "missing definition origin for {subject:?}")
            }
            Self::InvalidLexicalOwner(owner) => write!(f, "invalid lexical owner {owner:?}"),
            Self::InvalidSourceDeclaration(reason) => {
                write!(f, "invalid persistent source declaration: {reason}")
            }
            Self::MissingExactIdentity { context } => {
                write!(f, "missing persistent exact type identity for {context}")
            }
            Self::ExactIdentityMismatch(exact) => write!(
                f,
                "source nominal exact identity {exact} disagrees between Export and LocalConcrete HIR"
            ),
            Self::MissingConcreteType(exact) => {
                write!(f, "exact type {exact} is absent from LocalConcrete HIR")
            }
            Self::MissingLocalSupport(exact) => write!(
                f,
                "exact type {exact} needs local source support that is not exported"
            ),
            Self::InvalidSourceShape {
                declaration,
                reason,
            } => write!(f, "invalid source shape for {declaration:?}: {reason}"),
            Self::InvalidFact { exact, reason } => {
                write!(f, "invalid exact facts for {exact}: {reason}")
            }
            Self::InvalidRepresentation {
                declaration,
                reason,
            } => write!(f, "invalid representation for {declaration:?}: {reason}"),
            Self::InvalidInheritance { exact, reason } => {
                write!(f, "invalid inheritance interface for {exact}: {reason}")
            }
            Self::InvalidTable { table, reason } => write!(f, "invalid {table} table: {reason}"),
        }
    }
}

impl std::error::Error for CrossConeTypeSemanticsProductionError {}
