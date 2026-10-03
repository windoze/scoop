use super::*;
use std::fmt;

#[derive(Debug)]
pub enum TypeSemanticsSectionResolutionError<E> {
    Facts(ExactTypeFactsTableResolutionError<E>),
    Representation(Box<NominalRepresentationTableResolutionError<E>>),
    Inheritance(Box<InheritanceInterfaceResolutionError<E>>),
    Selected(SelectedTypeUseResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for TypeSemanticsSectionResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Facts(e) => write!(f, "type section field 1: {e}"),
            Self::Representation(e) => write!(f, "type section field 2: {e}"),
            Self::Inheritance(e) => write!(f, "type section field 3: {e}"),
            Self::Selected(e) => write!(f, "type section field 8: {e}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSemanticsSectionResolutionError<E> {}
