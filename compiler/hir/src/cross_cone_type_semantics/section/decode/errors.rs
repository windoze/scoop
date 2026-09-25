use super::*;
use std::fmt;

#[derive(Debug)]
pub enum TypeSemanticsSectionResolutionError<E> {
    Resource(WireError),
    Facts(ExactTypeFactsTableResolutionError<E>),
    Representation(Box<NominalRepresentationTableResolutionError<E>>),
    Inheritance(Box<InheritanceInterfaceResolutionError<E>>),
    Declarations(Box<ProtectedDeclarationResolutionError<E>>),
    Sources(Box<ProtectedSourceResolutionError<E>>),
    Defaults(Box<ProtectedDefaultTemplateTableResolutionError<E>>),
    Origins(Box<ExportDefinitionSourceSetValidationError<E>>),
    Selected(SelectedTypeUseResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for TypeSemanticsSectionResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Facts(e) => write!(f, "type section field 1: {e}"),
            Self::Representation(e) => write!(f, "type section field 2: {e}"),
            Self::Inheritance(e) => write!(f, "type section field 3: {e}"),
            Self::Declarations(e) => write!(f, "type section field 4: {e}"),
            Self::Sources(e) => write!(f, "type section field 5: {e}"),
            Self::Defaults(e) => write!(f, "type section field 6: {e}"),
            Self::Origins(e) => write!(f, "type section field 7: {e}"),
            Self::Selected(e) => write!(f, "type section field 8: {e}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSemanticsSectionResolutionError<E> {}
