use crate::{
    ExportDefinitionSourceSemanticValidationError, ProtectedDefaultBodyClosureError,
    ProtectedDefaultDomainCoverageError, ProtectedDefaultWitnessProfileError,
    ProtectedDefaultWitnessSourceError,
};
use scoop_wire::{WireError, cbor::EncodeError};

#[derive(Debug)]
pub enum ProtectedDefaultReferenceSemanticError<E> {
    OwnerProfile(ProtectedDefaultWitnessSourceError<E>),
    Body(ProtectedDefaultBodyClosureError<ProtectedDefaultReferenceAccessError<E>>),
}
#[derive(Debug)]
pub enum ProtectedDefaultReferenceAccessError<E> {
    Resource(WireError),
    Encoding(EncodeError),
    Origin(ExportDefinitionSourceSemanticValidationError<E>),
    Source(E),
    Witness(ProtectedDefaultWitnessProfileError),
    Coverage(ProtectedDefaultDomainCoverageError<E>),
    TemplateKey,
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultReferenceSemanticError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnerProfile(error) => error.fmt(f),
            Self::Body(error) => error.fmt(f),
        }
    }
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedDefaultReferenceAccessError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Witness(error) => error.fmt(f),
            Self::Coverage(error) => error.fmt(f),
            Self::TemplateKey => {
                f.write_str("default reference use has a different checked template key")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedDefaultReferenceSemanticError<E>
{
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedDefaultReferenceAccessError<E> {}
