use super::*;

#[derive(Debug)]
pub enum SelectedTypeUseBuildError {
    Encoding(scoop_wire::cbor::EncodeError),
    Duplicate { index: usize },
}
impl fmt::Display for SelectedTypeUseBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(error) => error.fmt(f),
            Self::Duplicate { index } => {
                write!(f, "duplicate selected type target at index {index}")
            }
        }
    }
}
impl std::error::Error for SelectedTypeUseBuildError {}

#[derive(Debug)]
pub enum SelectedTypeUseResolutionError<E> {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Reference(E),
    Duplicate { index: usize },
    NonCanonicalOrder { index: usize },
}
impl<E: fmt::Display> fmt::Display for SelectedTypeUseResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Reference(error) => write!(f, "invalid selected type target reference: {error}"),
            Self::Duplicate { index } => {
                write!(f, "duplicate selected type target at index {index}")
            }
            Self::NonCanonicalOrder { index } => {
                write!(f, "noncanonical selected type target at index {index}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for SelectedTypeUseResolutionError<E> {}
