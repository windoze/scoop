use crate::concrete::ExecutableExpressionPosition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDependencyCallSiteBuildError {
    EmptyWitnessIndices,
    WitnessIndexOrder { index: usize },
    DuplicatePosition(ExecutableExpressionPosition),
    PositionOrder { index: usize },
}

impl std::fmt::Display for HirDependencyCallSiteBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid dependency call site: {self:?}")
    }
}

impl std::error::Error for HirDependencyCallSiteBuildError {}

#[derive(Debug)]
pub enum HirDependencyCallSiteResolutionError<E> {
    Resource(scoop_wire::WireError),
    Identity(E),
    Origin(scoop_identity::SourceOriginResolutionError<E>),
    Shape(HirDependencyCallSiteBuildError),
}

impl<E: std::fmt::Display> std::fmt::Display for HirDependencyCallSiteResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::Shape(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for HirDependencyCallSiteResolutionError<E> {}
