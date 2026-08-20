use crate::InheritanceQueryError;
use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceSlotSchemaSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    Inheritance(InheritanceQueryError),
    SlotIdentity(PersistentDispatchSlotId),
    InterfaceSource(PersistentExactTypeId),
    InvalidOverride {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        overridden: PersistentDispatchSlotId,
    },
    RoleCoverage(PersistentExactTypeId),
    BasePrefix(PersistentExactTypeId),
    InheritedSlots(PersistentExactTypeId),
    InterfaceOrder {
        owner: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    NewSlotOwner {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
}
impl<E: fmt::Display> fmt::Display for InheritanceSlotSchemaSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => write!(f, "invalid slot foundation: {error}"),
            Self::Inheritance(error) => error.fmt(f),
            Self::InterfaceSource(owner) => write!(
                f,
                "interface {owner} source owner or direct parents disagree with its inheritance graph"
            ),
            Self::InvalidOverride {
                owner,
                slot,
                overridden,
            } => write!(
                f,
                "interface {owner} slot {slot} cannot override non-inherited or differently-typed slot {overridden}"
            ),
            Self::SlotIdentity(slot) => write!(
                f,
                "slot {slot} does not match its source declaration owner and role"
            ),
            Self::RoleCoverage(owner) => {
                write!(f, "slot schema roles do not cover exact owner {owner}")
            }
            Self::BasePrefix(owner) => write!(
                f,
                "vtable for {owner} does not preserve the complete base prefix"
            ),
            Self::InheritedSlots(owner) => write!(
                f,
                "interface {owner} does not match its replayed source slot sequence"
            ),
            Self::InterfaceOrder { owner, interface } => write!(
                f,
                "interface schema {interface} in {owner} differs from its provider sequence"
            ),
            Self::NewSlotOwner { owner, slot } => write!(
                f,
                "new slot {slot} in {owner} belongs to another source declaration owner"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceSlotSchemaSemanticError<E> {}
