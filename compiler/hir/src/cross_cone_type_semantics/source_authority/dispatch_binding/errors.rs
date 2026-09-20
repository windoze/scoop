use std::fmt;

use super::*;

#[derive(Debug)]
pub enum InheritanceDispatchBindingError {
    Resource(WireError),
    Foundation(TypeFoundationBindingError),
    Inventory(&'static str),
    MissingOwner(PersistentExactTypeId),
    MissingInterface(PersistentExactTypeId),
    MissingFunction(PersistentFunctionId),
    MissingProperty(PersistentPropertyId),
    MissingSlot(PersistentDispatchSlotId),
    MissingCallable(InheritanceCallableDeclarationV1),
    MissingSelection {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    SlotRole(PersistentDispatchSlotId),
    AccessorRole(InheritanceCallableDeclarationV1),
    ForeignDeclaration(InheritanceCallableDeclarationV1),
    DefinitionOrigin(InheritanceCallableDeclarationV1),
    Access {
        declaration: InheritanceCallableDeclarationV1,
        reason: String,
    },
}

impl From<WireError> for InheritanceDispatchBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<TypeFoundationBindingError> for InheritanceDispatchBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(other),
        }
    }
}

impl fmt::Display for InheritanceDispatchBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Inventory(table) => write!(f, "dispatch source inventory mismatch: {table}"),
            Self::MissingOwner(id) => write!(f, "missing inheritance source owner {id}"),
            Self::MissingInterface(id) => write!(f, "missing interface dispatch source {id}"),
            Self::MissingFunction(id) => {
                write!(f, "function {id} is absent from the owning foundation")
            }
            Self::MissingProperty(id) => {
                write!(f, "property {id} is absent from the owning foundation")
            }
            Self::MissingSlot(id) => {
                write!(f, "dispatch slot {id} is absent from the owning foundation")
            }
            Self::MissingCallable(id) => write!(f, "missing dispatch source callable {id:?}"),
            Self::MissingSelection { owner, slot } => {
                write!(f, "missing dispatch source selection for {owner}/{slot}")
            }
            Self::SlotRole(id) => write!(f, "dispatch slot {id} has the wrong declaration role"),
            Self::AccessorRole(id) => {
                write!(f, "dispatch source {id:?} has the wrong accessor role")
            }
            Self::ForeignDeclaration(id) => {
                write!(f, "dispatch source {id:?} belongs to a different provider")
            }
            Self::DefinitionOrigin(id) => write!(
                f,
                "dispatch source {id:?} differs from its foundation definition origin"
            ),
            Self::Access {
                declaration,
                reason,
            } => write!(
                f,
                "invalid dispatch source access for {declaration:?}: {reason}"
            ),
        }
    }
}

impl std::error::Error for InheritanceDispatchBindingError {}
