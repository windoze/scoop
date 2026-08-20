use std::fmt;

use scoop_identity::PersistentPropertyAccessorId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyCapabilityBuildError {
    DuplicateAccessor(PersistentPropertyAccessorId),
}

impl fmt::Display for PropertyCapabilityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAccessor(accessor) => {
                write!(formatter, "property reuses accessor identity {accessor}")
            }
        }
    }
}

impl std::error::Error for PropertyCapabilityBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum PropertyCapabilityResolutionError<E> {
    Getter(E),
    Setter(E),
    Capability(PropertyCapabilityBuildError),
}

impl<E: fmt::Display> fmt::Display for PropertyCapabilityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Getter(error) => write!(formatter, "invalid property getter: {error}"),
            Self::Setter(error) => write!(formatter, "invalid property setter: {error}"),
            Self::Capability(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PropertyCapabilityResolutionError<E> {}
