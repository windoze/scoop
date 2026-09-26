use crate::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceInterfaceBuildError {
    SlotClosure,
    OwnerOrder,
}
impl fmt::Display for InheritanceInterfaceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SlotClosure => {
                "inheritance slot contracts differ from the union of the owner slot schemas"
            }
            Self::OwnerOrder => "inheritance owners are duplicate or out of canonical order",
        })
    }
}
impl std::error::Error for InheritanceInterfaceBuildError {}

#[derive(Debug)]
pub enum InheritanceInterfaceResolutionError<E> {
    Resource(WireError),
    Edges(InheritanceEdgeResolutionError<E>),
    Slots(InheritanceSlotResolutionError<E>),
    Members(ProtectedDeclarationResolutionError<E>),
    Schemas(InheritanceSlotSchemaResolutionError<E>),
    Build(InheritanceInterfaceBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceInterfaceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Edges(error) => error.fmt(f),
            Self::Slots(error) => error.fmt(f),
            Self::Members(error) => error.fmt(f),
            Self::Schemas(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceInterfaceResolutionError<E> {}

#[derive(Debug)]
pub enum InheritanceInterfaceSemanticError<E> {
    Foundation(E),
    Slot(InheritanceSlotContractSemanticError<E>),
    SourceContract,
    SlotSelection,
}
impl<E: fmt::Display> fmt::Display for InheritanceInterfaceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Foundation(error) => return error.fmt(f),
            Self::Slot(error) => return error.fmt(f),
            Self::SourceContract => {
                "inheritance callable contract differs from the actual source declaration"
            }
            Self::SlotSelection => {
                "inheritance slot implementation differs from the actual resolved selection"
            }
        })
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceInterfaceSemanticError<E> {}
