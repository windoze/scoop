use crate::{
    DeclarationAccessSourceResolutionError, ExportConstValueResolutionError,
    ProtectedPropertyResolutionError,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalSupportPropertyBuildError {
    Owner,
    Setter,
    ConstIdentity,
    ConstOrigin,
}
impl fmt::Display for NominalSupportPropertyBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Owner => "nominal property support requires the actual lexical owner",
            Self::Setter => "nominal property setter access differs from its property site",
            Self::ConstIdentity => "const support names another property",
            Self::ConstOrigin => "const support has another definition origin",
        })
    }
}
impl std::error::Error for NominalSupportPropertyBuildError {}

#[derive(Debug)]
pub enum NominalSupportPropertyResolutionError<E> {
    Resource(scoop_wire::WireError),
    Identity(E),
    Source(DeclarationAccessSourceResolutionError<E>),
    Runtime(ProtectedPropertyResolutionError<E>),
    Const(ExportConstValueResolutionError<E>),
    Property(NominalSupportPropertyBuildError),
}
impl<E: fmt::Display> fmt::Display for NominalSupportPropertyResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Identity(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Runtime(e) => e.fmt(f),
            Self::Const(e) => e.fmt(f),
            Self::Property(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalSupportPropertyResolutionError<E>
{
}
