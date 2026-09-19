use super::*;
use scoop_identity::{PersistentExactTypeId, PersistentTypeId};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum TypeSectionExportValidationError<E> {
    Resource(WireError),
    Source(E),
    Provider,
    DependencyOrder,
    SourceRootOrder,
    SourceRoot(SourceNominalId),
    InheritanceInventory,
    FactsInventory,
    DependencyFactOrder,
    DependencyFact(PersistentExactTypeId),
    MissingNominalSupport(PersistentTypeId),
    PublicOverlap,
    FactShape(PersistentExactTypeId),
    FactType(Box<InheritanceSlotContractSemanticError<E>>),
    Facts(Box<ExactTypeFactsSemanticError<E>>),
    Representation(Box<NominalRepresentationSourceSemanticError<E>>),
    Graph(Box<InheritanceGraphError<E>>),
    Protected(Box<ProtectedDeclarationSemanticError<E>>),
    Inheritance(Box<InheritanceInterfaceSemanticError<E>>),
    Sources(Box<ProtectedSourceClosureError<E>>),
    Defaults(Box<ProtectedDefaultTableSemanticError<E>>),
    Origins(Box<TypeDefinitionSourceClosureError<E>>),
}
impl<E> From<WireError> for TypeSectionExportValidationError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Display> fmt::Display for TypeSectionExportValidationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Provider => f.write_str("type section public/source provider mismatch"),
            Self::DependencyOrder => f.write_str(
                "type section terminal providers repeat, are local, or are not canonical",
            ),
            Self::SourceRootOrder => {
                f.write_str("type section source roots repeat or are not canonical")
            }
            Self::SourceRoot(id) => {
                write!(f, "type section source root is not locally owned: {id:?}")
            }
            Self::InheritanceInventory => f.write_str(
                "type section inheritance differs from independent source edge inventory",
            ),
            Self::FactsInventory => {
                f.write_str("type section exact facts differ from independent local inventory")
            }
            Self::DependencyFactOrder => {
                f.write_str("type section external fact ownership repeats or is not canonical")
            }
            Self::DependencyFact(id) => write!(
                f,
                "type section fact {id} lacks its declared terminal owner"
            ),
            Self::MissingNominalSupport(id) => write!(
                f,
                "type section source nominal {id} lacks complete local support"
            ),
            Self::PublicOverlap => {
                f.write_str("type section and checked public source contracts disagree")
            }
            Self::FactShape(exact) => write!(
                f,
                "type facts for {exact} do not describe the checked source representation"
            ),
            Self::FactType(error) => error.fmt(f),
            Self::Facts(e) => e.fmt(f),
            Self::Representation(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::Protected(e) => e.fmt(f),
            Self::Inheritance(e) => e.fmt(f),
            Self::Sources(e) => e.fmt(f),
            Self::Defaults(e) => e.fmt(f),
            Self::Origins(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSectionExportValidationError<E> {}
