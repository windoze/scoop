use crate::{DeclarationAccessSourceResolutionError, ProtectedCallableInterfaceResolutionError};
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedPropertyBuildError {
    Access,
    Owner,
    Accessor,
    SetterAccess,
    Representation,
    MissingSlot,
}
impl fmt::Display for ProtectedPropertyBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Access => "protected property requires declared protected visibility",
            Self::Owner => "protected property and accessor owner chains disagree",
            Self::Accessor => "property getter and setter must be distinct typed accessors",
            Self::SetterAccess => "protected property setter has an invalid source access",
            Self::Representation => "protected class property cannot use const representation",
            Self::MissingSlot => "abstract protected property requires a slot relation",
        })
    }
}
impl std::error::Error for ProtectedPropertyBuildError {}

#[derive(Debug)]
pub enum ProtectedPropertyResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Access(DeclarationAccessSourceResolutionError<E>),
    Slots(ProtectedCallableInterfaceResolutionError<E>),
    Property(ProtectedPropertyBuildError),
}
impl<E: fmt::Display> fmt::Display for ProtectedPropertyResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Access(error) => error.fmt(f),
            Self::Slots(error) => error.fmt(f),
            Self::Property(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedPropertyResolutionError<E> {}
