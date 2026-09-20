use super::*;
use std::fmt;

#[derive(Debug)]
pub enum NominalNestedBindingError {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Foundation(Box<TypeFoundationBindingError>),
    Nominal(Box<NominalSourceBindingError>),
    Member(Box<NominalMemberBindingError>),
    Constructor(Box<NominalConstructorBindingError>),
    Graph(Box<InheritanceGraphError<TypeFoundationBindingError>>),
    Concrete(Box<NestedSourceSemanticError<NominalSourceBindingError>>),
    Protocol(Box<ProtectedSourceSemanticError<NominalParameterBindingError>>),
    Contract {
        declaration: NestedSupportDeclarationV1,
        field: &'static str,
    },
    MissingProtocol(CallableTemplateOrigin),
    DuplicateProtocol(CallableTemplateOrigin),
}
impl From<WireError> for NominalNestedBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
macro_rules! source_error {
    ($ty:ident, $variant:ident) => {
        impl From<$ty> for NominalNestedBindingError {
            fn from(error: $ty) -> Self {
                match error {
                    $ty::Resource(e) => Self::Resource(e),
                    other => Self::$variant(Box::new(other)),
                }
            }
        }
    };
}
source_error!(TypeFoundationBindingError, Foundation);
source_error!(NominalSourceBindingError, Nominal);
source_error!(NominalMemberBindingError, Member);
source_error!(NominalConstructorBindingError, Constructor);
impl NominalNestedBindingError {
    pub(super) fn from_graph(error: InheritanceGraphError<TypeFoundationBindingError>) -> Self {
        match error {
            InheritanceGraphError::Resource(e) => Self::Resource(e),
            InheritanceGraphError::Foundation(TypeFoundationBindingError::Resource(e)) => {
                Self::Resource(e)
            }
            other => Self::Graph(Box::new(other)),
        }
    }
    pub(super) fn from_concrete(
        error: NestedSourceSemanticError<NominalSourceBindingError>,
    ) -> Self {
        match error {
            NestedSourceSemanticError::Resource(e) => Self::Resource(e),
            other => Self::Concrete(Box::new(other)),
        }
    }
    pub(super) fn from_protocol(
        error: ProtectedSourceSemanticError<NominalParameterBindingError>,
    ) -> Self {
        match error {
            ProtectedSourceSemanticError::Resource(e) => Self::Resource(e),
            ProtectedSourceSemanticError::Foundation(NominalParameterBindingError::Resource(e)) => {
                Self::Resource(e)
            }
            other => Self::Protocol(Box::new(other)),
        }
    }
}
impl fmt::Display for NominalNestedBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Nominal(e) => e.fmt(f),
            Self::Member(e) => e.fmt(f),
            Self::Constructor(e) => e.fmt(f),
            Self::Graph(e) => e.fmt(f),
            Self::Concrete(e) => e.fmt(f),
            Self::Protocol(e) => e.fmt(f),
            Self::Contract { declaration, field } => write!(
                f,
                "nested source {declaration:?} differs from its bound {field} contract"
            ),
            Self::MissingProtocol(owner) => {
                write!(f, "nested source lacks parameter protocol {owner:?}")
            }
            Self::DuplicateProtocol(owner) => {
                write!(f, "nested source repeats parameter protocol {owner:?}")
            }
        }
    }
}
impl std::error::Error for NominalNestedBindingError {}
