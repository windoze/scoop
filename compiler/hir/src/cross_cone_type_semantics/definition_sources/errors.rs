use scoop_wire::{WireError, WirePath};
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub enum TypeDefinitionSourceClosureError<E> {
    Missing {
        path: WirePath,
        insertion_index: usize,
    },
    Extra {
        index: usize,
    },
    Source {
        path: WirePath,
        error: E,
    },
    Resource(WireError),
}
impl<E> From<WireError> for TypeDefinitionSourceClosureError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Display> fmt::Display for TypeDefinitionSourceClosureError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing {
                path,
                insertion_index,
            } => write!(
                f,
                "type definition origin at {path:?} is absent at canonical index {insertion_index}"
            ),
            Self::Extra { index } => write!(
                f,
                "type definition origin at field 7 index {index} has no inline use"
            ),
            Self::Source { path, error } => {
                write!(f, "invalid type definition origin use at {path:?}: {error}")
            }
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeDefinitionSourceClosureError<E> {}
