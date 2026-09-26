use crate::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InheritanceInterfaceBuildError {
    ConstructorAccess,
    ConstructorGeneric,
    ConstructorOrder,
    ConstructorInMembers,
    SlotClosure,
    OwnerOrder,
}
impl fmt::Display for InheritanceInterfaceBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ConstructorAccess => {
                "inheritance constructor entry must be declared public or protected"
            }
            Self::ConstructorGeneric => {
                "inheritance constructor entry requires a parameter-free source contract"
            }
            Self::ConstructorOrder => {
                "inheritance constructors are duplicate or out of canonical order"
            }
            Self::ConstructorInMembers => {
                "constructors belong in their separate inheritance constructor field"
            }
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
    Encoding(scoop_wire::cbor::EncodeError),
    Edges(InheritanceEdgeResolutionError<E>),
    Constructor(ProtectedCallableInterfaceResolutionError<E>),
    Slots(InheritanceSlotResolutionError<E>),
    Members(ProtectedDeclarationResolutionError<E>),
    Schemas(InheritanceSlotSchemaResolutionError<E>),
    Build(InheritanceInterfaceBuildError),
}
impl<E: fmt::Display> fmt::Display for InheritanceInterfaceResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Edges(error) => error.fmt(f),
            Self::Constructor(error) => error.fmt(f),
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
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Foundation(E),
    Schema(InheritanceSlotSchemaSemanticError<E>),
    Slot(InheritanceSlotContractSemanticError<E>),
    Constructor(NominalSupportCallableSemanticError<E>),
    Inventory,
    Edges,
    SourceContract,
    ConstructorOwner,
    ProtectedDeclaration,
    SlotSelection,
}
impl<E: fmt::Display> fmt::Display for InheritanceInterfaceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Resource(error) => return error.fmt(f),
            Self::Encoding(error) => return error.fmt(f),
            Self::Foundation(error) => return error.fmt(f),
            Self::Schema(error) => return error.fmt(f),
            Self::Slot(error) => return error.fmt(f),
            Self::Constructor(error) => return error.fmt(f),
            Self::Inventory => "inheritance table differs from the complete source inventory",
            Self::Edges => "inheritance interface edges differ from the checked graph",
            Self::SourceContract => {
                "inheritance callable contract differs from the actual source declaration"
            }
            Self::ConstructorOwner => "inheritance constructor belongs to another nominal owner",
            Self::ProtectedDeclaration => {
                "inheritance declaration differs from the checked protected source table"
            }
            Self::SlotSelection => {
                "inheritance slot implementation differs from the actual resolved selection"
            }
        })
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceInterfaceSemanticError<E> {}
