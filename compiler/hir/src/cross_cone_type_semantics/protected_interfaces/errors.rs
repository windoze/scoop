use crate::{
    BinderListValidationError, CallableSourceEffectsBuildError,
    DeclarationAccessSourceResolutionError, MeteredInterfaceResolutionError,
    SourceParameterListValidationError,
};
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedCallableInterfaceBuildError {
    DeclarationKind,
    SourceInterface,
    Binders,
    Modality,
    MissingSlot,
    Execution,
    Receiver,
    Access,
    Owner,
    SlotOrder { index: usize },
}
impl fmt::Display for ProtectedCallableInterfaceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclarationKind => {
                f.write_str("protected callable has the wrong declaration kind")
            }
            Self::SourceInterface => {
                f.write_str("protected source-interface reference differs from its declaration")
            }
            Self::Binders => {
                f.write_str("protected callable binder shape differs from its declaration kind")
            }
            Self::Modality => f.write_str(
                "protected callable modality, effects, or slot relations are inconsistent",
            ),
            Self::MissingSlot => {
                f.write_str("open or abstract protected callable requires a slot relation")
            }
            Self::Execution => {
                f.write_str("constructor and accessor source interfaces must be synchronous")
            }
            Self::Receiver => {
                f.write_str("protected nominal source callable cannot have an extension receiver")
            }
            Self::Access => {
                f.write_str("protected interface requires declared protected visibility")
            }
            Self::Owner => {
                f.write_str("protected callable owner differs from the source owner chain")
            }
            Self::SlotOrder { index } => write!(
                f,
                "duplicate or noncanonical protected slot reference at index {index}"
            ),
        }
    }
}
impl std::error::Error for ProtectedCallableInterfaceBuildError {}

#[derive(Debug)]
pub enum ProtectedCallableInterfaceResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Effects(CallableSourceEffectsBuildError),
    Binders(MeteredInterfaceResolutionError<BinderListValidationError<E>>),
    Parameters(MeteredInterfaceResolutionError<SourceParameterListValidationError<E>>),
    Source(DeclarationAccessSourceResolutionError<E>),
    Interface(ProtectedCallableInterfaceBuildError),
}
impl<E: fmt::Display> fmt::Display for ProtectedCallableInterfaceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Effects(error) => error.fmt(f),
            Self::Binders(error) => error.fmt(f),
            Self::Parameters(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Interface(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProtectedCallableInterfaceResolutionError<E>
{
}
