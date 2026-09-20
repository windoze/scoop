use crate::{CanonicalPersistentIdSetValidationError, PersistentAccessResolutionError};
use scoop_identity::PersistentGenericTypeId;
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceAccessBuildError {
    GenericConstraintsOnEmpty,
    AccessorOwner,
    SlotForConstructor,
}
impl fmt::Display for DefaultSourceAccessBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GenericConstraintsOnEmpty => {
                "empty default source domain cannot carry generic subclass constraints"
            }
            Self::AccessorOwner => "an accessor cannot own a default source witness",
            Self::SlotForConstructor => "a constructor cannot have a default source slot domain",
        })
    }
}
impl std::error::Error for DefaultSourceAccessBuildError {}

#[derive(Debug)]
pub enum DefaultSourceAccessResolutionError<E> {
    Resource(WireError),
    Domain(PersistentAccessResolutionError<E>),
    GenericSubclasses(CanonicalPersistentIdSetValidationError<PersistentGenericTypeId, E>),
    Owner(E),
    Build(DefaultSourceAccessBuildError),
}
impl<E> DefaultSourceAccessResolutionError<E> {
    pub fn resource_error(&self) -> Option<&WireError> {
        match self {
            Self::Resource(error)
            | Self::Domain(PersistentAccessResolutionError::Resource(error)) => Some(error),
            _ => None,
        }
    }
}
impl<E: fmt::Display> fmt::Display for DefaultSourceAccessResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Domain(error) => error.fmt(f),
            Self::GenericSubclasses(error) => error.fmt(f),
            Self::Owner(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DefaultSourceAccessResolutionError<E> {}
