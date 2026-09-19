use scoop_identity::{CanonicalIdentifierError, SourceOriginResolutionError};
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedSourceBuildError {
    AccessorOwner,
    TooMany,
    DuplicateName { position: u32 },
    MultipleVarargs,
    TemplateOwner { position: u32 },
    DuplicateDefault,
    DefaultIndex,
    DuplicateOwner,
    NonCanonicalOrder,
    DefaultClosure,
}
impl fmt::Display for ProtectedSourceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessorOwner => {
                f.write_str("an accessor has no source parameter/default protocol")
            }
            Self::TooMany => f.write_str("protected source table exceeds u32 indexing"),
            Self::DuplicateName { position } => {
                write!(f, "duplicate source parameter name at {position}")
            }
            Self::MultipleVarargs => {
                f.write_str("a source protocol has multiple vararg parameters")
            }
            Self::TemplateOwner { position } => write!(
                f,
                "protected default key differs from its owner or parameter {position}"
            ),
            Self::DuplicateDefault => f.write_str("duplicate protected default key"),
            Self::DefaultIndex => {
                f.write_str("protected default key/index is absent from its own table")
            }
            Self::DuplicateOwner => f.write_str("duplicate protected source protocol owner"),
            Self::NonCanonicalOrder => {
                f.write_str("protected source protocol table is not in canonical owner order")
            }
            Self::DefaultClosure => {
                f.write_str("protected source protocols and default keys do not close exactly")
            }
        }
    }
}
impl std::error::Error for ProtectedSourceBuildError {}

#[derive(Debug)]
pub enum ProtectedSourceResolutionError<E> {
    Resource(WireError),
    Foundation(E),
    Name(CanonicalIdentifierError),
    Origin(SourceOriginResolutionError<E>),
    Encoding(scoop_wire::cbor::EncodeError),
    Build(ProtectedSourceBuildError),
}
impl<E: fmt::Display> fmt::Display for ProtectedSourceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Name(e) => e.fmt(f),
            Self::Origin(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedSourceResolutionError<E> {}

#[derive(Debug)]
pub enum ProtectedSourceIndexError {
    Resource(WireError),
    Build(ProtectedSourceBuildError),
}
impl fmt::Display for ProtectedSourceIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ProtectedSourceIndexError {}
