use super::*;
use scoop_identity::PersistentExactTypeId;
use std::fmt;

#[derive(Debug)]
pub enum NominalDispatchBindingError {
    Resource(WireError),
    Source(Box<NominalNestedBindingError>),
    Protected(Box<ProtectedDeclarationBindingError>),
    Slot(Box<InheritanceSlotSourceBindingError>),
    Signature {
        declaration: InheritanceCallableDeclarationV1,
        error: Box<InheritanceSlotContractSemanticError<TypeFoundationBindingError>>,
    },
    FoundationMismatch,
    CoreUnit,
    Inventory {
        owner: PersistentExactTypeId,
        field: &'static str,
    },
    ProtectedOwner,
    Callable {
        declaration: InheritanceCallableDeclarationV1,
        field: &'static str,
    },
}
impl From<WireError> for NominalDispatchBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalNestedBindingError> for NominalDispatchBindingError {
    fn from(error: NominalNestedBindingError) -> Self {
        match error {
            NominalNestedBindingError::Resource(e) => Self::Resource(e),
            other => Self::Source(Box::new(other)),
        }
    }
}
impl NominalDispatchBindingError {
    pub(super) fn from_slot(error: InheritanceSlotSourceBindingError) -> Self {
        match error {
            InheritanceSlotSourceBindingError::Resource(e) => Self::Resource(e),
            other => Self::Slot(Box::new(other)),
        }
    }
    pub(super) fn signature(
        declaration: InheritanceCallableDeclarationV1,
        error: InheritanceSlotContractSemanticError<TypeFoundationBindingError>,
    ) -> Self {
        match error {
            InheritanceSlotContractSemanticError::Resource(e)
            | InheritanceSlotContractSemanticError::Foundation(
                TypeFoundationBindingError::Resource(e),
            ) => Self::Resource(e),
            other => Self::Signature {
                declaration,
                error: Box::new(other),
            },
        }
    }
}
impl fmt::Display for NominalDispatchBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Protected(e) => e.fmt(f),
            Self::Slot(e) => e.fmt(f),
            Self::Signature { error, .. } => error.fmt(f),
            Self::FoundationMismatch => {
                f.write_str("dispatch and nominal sources require the same bound foundation")
            }
            Self::CoreUnit => f.write_str("dispatch and nominal sources disagree on core Unit"),
            Self::ProtectedOwner => f.write_str("protected source has no unique nominal owner"),
            Self::Inventory { owner, field } => write!(
                f,
                "dispatch owner {owner:?} differs from complete {field} sources"
            ),
            Self::Callable { declaration, field } => write!(
                f,
                "dispatch callable {declaration:?} differs from complete source {field}"
            ),
        }
    }
}
impl std::error::Error for NominalDispatchBindingError {}
