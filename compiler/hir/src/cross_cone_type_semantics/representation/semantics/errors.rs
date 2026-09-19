use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalRepresentationSourceMismatchV1 {
    Provider,
    SourceKey,
    OwnerIdentity,
    SourceKind,
    AccessSource,
    AccessOwners,
    Access,
    Shape,
    PublicValueShape,
}
impl fmt::Display for NominalRepresentationSourceMismatchV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Provider => "representation source is not owned by the current provider",
            Self::SourceKey => "representation requires a parameter-free source nominal key",
            Self::OwnerIdentity => "representation owner disagrees with the source key",
            Self::SourceKind => "representation kind disagrees with the source declaration",
            Self::AccessSource => "representation access origin disagrees with its source key",
            Self::AccessOwners => {
                "representation access lexical owners disagree with the source key"
            }
            Self::Access => "representation access differs from the real source declaration",
            Self::Shape => "representation differs from the real source shape",
            Self::PublicValueShape => "representation differs from its public value source shape",
        })
    }
}
impl std::error::Error for NominalRepresentationSourceMismatchV1 {}

#[derive(Debug)]
pub enum NominalRepresentationSourceSemanticError<E> {
    Resource(WireError),
    Inventory(E),
    Missing {
        owner: PersistentTypeId,
    },
    Extra {
        index: usize,
        owner: PersistentTypeId,
    },
    Source {
        index: usize,
        owner: PersistentTypeId,
        error: E,
    },
    Record {
        index: usize,
        owner: PersistentTypeId,
        error: NominalRepresentationSourceMismatchV1,
    },
}
impl<E> From<WireError> for NominalRepresentationSourceSemanticError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Display> fmt::Display for NominalRepresentationSourceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => write!(f, "invalid representation source inventory: {error}"),
            Self::Missing { owner } => write!(f, "missing representation for source owner {owner}"),
            Self::Extra { index, owner } => {
                write!(f, "extra representation {owner} at index {index}")
            }
            Self::Source {
                index,
                owner,
                error,
            } => write!(
                f,
                "invalid representation source {owner} at index {index}: {error}"
            ),
            Self::Record {
                index,
                owner,
                error,
            } => write!(
                f,
                "invalid representation {owner} at index {index}: {error}"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalRepresentationSourceSemanticError<E>
{
}
