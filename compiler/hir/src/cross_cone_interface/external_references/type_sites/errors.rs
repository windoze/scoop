use super::*;
use scoop_wire::WireError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirDependencyTypeSiteBuildError {
    Resource(WireError),
    DuplicatePosition(ExecutableExpressionPosition, HirExpressionTypeRoleV1),
    PositionOrder { index: usize },
}

impl From<WireError> for HirDependencyTypeSiteBuildError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for HirDependencyTypeSiteBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid dependency type site: {self:?}")
    }
}
impl std::error::Error for HirDependencyTypeSiteBuildError {}

#[derive(Debug)]
pub enum HirDependencyTypeSiteResolutionError<E> {
    Resource(WireError),
    Identity(E),
    Origin(scoop_identity::SourceOriginResolutionError<E>),
    Shape(HirDependencyTypeSiteBuildError),
}

impl<E> From<WireError> for HirDependencyTypeSiteResolutionError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl<E: std::fmt::Display> std::fmt::Display for HirDependencyTypeSiteResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::Shape(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for HirDependencyTypeSiteResolutionError<E> {}
