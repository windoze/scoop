use super::*;
use crate::{NominalSupportCallableSemanticError, ProtectedCallableSemanticError};
use scoop_wire::{WireError, cbor::EncodeError};

#[derive(Debug)]
pub enum ProtectedSourceClosureError<E> {
    Resource(WireError),
    Encoding(EncodeError),
    OwnerInventory,
    ConflictingSource(CallableTemplateOrigin),
    DefaultClosure(ProtectedSourceIndexError),
    Protected {
        owner: CallableTemplateOrigin,
        error: Box<ProtectedCallableSemanticError<E>>,
    },
    Support {
        owner: CallableTemplateOrigin,
        error: Box<NominalSupportCallableSemanticError<E>>,
    },
    Protocol {
        owner: CallableTemplateOrigin,
        error: ProtectedSourceSemanticError<E>,
    },
}
impl<E: std::fmt::Display> std::fmt::Display for ProtectedSourceClosureError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f), Self::Encoding(error) => error.fmt(f),
            Self::DefaultClosure(error) => error.fmt(f),
            Self::OwnerInventory => f.write_str("protected source protocol owners do not equal the complete checked source-use inventory"),
            Self::ConflictingSource(owner) => write!(f, "source surfaces disagree on the complete callable contract for {owner:?}"),
            Self::Protected { owner, error } => write!(f, "invalid protected source {owner:?}: {error}"),
            Self::Support { owner, error } => write!(f, "invalid nominal support source {owner:?}: {error}"),
            Self::Protocol { owner, error } => write!(f, "invalid protected parameter protocol {owner:?}: {error}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedSourceClosureError<E> {}
