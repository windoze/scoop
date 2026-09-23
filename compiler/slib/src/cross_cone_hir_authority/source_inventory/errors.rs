use super::*;
use scoop_hir::{
    DefaultSourceNestedCallableQueryError, DefaultSourceTargetSubjectError,
    ExportDefaultTemplateKeyV1,
};
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceInventoryDeclaration {
    Nominal(SourceNominalId),
    Callable(CallableTemplateOrigin),
    Property(PropertyDeclarationId),
}

#[derive(Debug)]
pub enum CrossConeHirSourceInventoryError {
    Missing(SourceInventoryDeclaration),
    Unrelated(SourceInventoryDeclaration),
    Protocol(CallableTemplateOrigin),
    Default(ExportDefaultTemplateKeyV1),
    Declaration(Box<CrossConeHirNominalAuthorityError>),
    Target(Box<DefaultSourceTargetSubjectError>),
    Nested(Box<DefaultSourceNestedCallableQueryError>),
    Resource(WireError),
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<CrossConeHirNominalAuthorityError> for Error {
    fn from(error: CrossConeHirNominalAuthorityError) -> Self {
        Self::Declaration(Box::new(error))
    }
}
impl From<DefaultSourceTargetSubjectError> for Error {
    fn from(error: DefaultSourceTargetSubjectError) -> Self {
        Self::Target(Box::new(error))
    }
}
impl From<DefaultSourceNestedCallableQueryError> for Error {
    fn from(error: DefaultSourceNestedCallableQueryError) -> Self {
        Self::Nested(Box::new(error))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(id) => write!(f, "required shared source declaration is absent: {id:?}"),
            Self::Unrelated(id) => write!(
                f,
                "shared source support is unreachable from public roots: {id:?}"
            ),
            Self::Protocol(id) => write!(
                f,
                "shared source callable has no parameter protocol: {id:?}"
            ),
            Self::Default(id) => write!(f, "shared source parameter has no default body: {id:?}"),
            Self::Declaration(e) => e.fmt(f),
            Self::Target(e) => e.fmt(f),
            Self::Nested(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
