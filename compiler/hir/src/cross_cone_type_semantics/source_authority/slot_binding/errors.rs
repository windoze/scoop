use super::*;
use std::fmt;

#[derive(Debug)]
pub enum InheritanceSlotSourceBindingError {
    Resource(scoop_wire::WireError),
    Foundation(Box<TypeFoundationBindingError>),
    Dispatch(Box<InheritanceDispatchBindingError>),
    Graph(Box<InheritanceGraphError<TypeFoundationBindingError>>),
    Identity(String),
    CoreUnit,
}
impl InheritanceSlotSourceBindingError {
    pub(super) fn from_graph(error: InheritanceGraphError<TypeFoundationBindingError>) -> Self {
        match error {
            InheritanceGraphError::Resource(error) => Self::Resource(error),
            other => Self::Graph(Box::new(other)),
        }
    }
}
impl From<scoop_wire::WireError> for InheritanceSlotSourceBindingError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for InheritanceSlotSourceBindingError {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(Box::new(other)),
        }
    }
}
impl From<InheritanceDispatchBindingError> for InheritanceSlotSourceBindingError {
    fn from(error: InheritanceDispatchBindingError) -> Self {
        match error {
            InheritanceDispatchBindingError::Resource(error) => Self::Resource(error),
            other => Self::Dispatch(Box::new(other)),
        }
    }
}
impl fmt::Display for InheritanceSlotSourceBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Dispatch(error) => error.fmt(f),
            Self::Graph(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::CoreUnit => f.write_str("slot Unit role differs from the bound trusted core"),
        }
    }
}
impl std::error::Error for InheritanceSlotSourceBindingError {}
