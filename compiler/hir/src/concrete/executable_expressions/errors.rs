use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutableExpressionStructureError {
    Resource(WireError),
    DuplicateRoot(CallableMaterialization),
    ExpressionIndexOverflow(CallableMaterialization),
    MissingLambda(LambdaId),
    MissingAnonymousFunction(AnonymousFunctionId),
    MissingCallableReference(CallableReferenceId),
}

impl From<WireError> for ExecutableExpressionStructureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExecutableExpressionStructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid executable HIR structure: {self:?}")
    }
}

impl std::error::Error for ExecutableExpressionStructureError {}

#[derive(Debug)]
pub enum ExecutableExpressionVisitError<E> {
    Structure(ExecutableExpressionStructureError),
    Visitor(E),
}

impl<E> From<ExecutableExpressionStructureError> for ExecutableExpressionVisitError<E> {
    fn from(error: ExecutableExpressionStructureError) -> Self {
        Self::Structure(error)
    }
}

impl<E> From<WireError> for ExecutableExpressionVisitError<E> {
    fn from(error: WireError) -> Self {
        Self::Structure(error.into())
    }
}

impl<E: std::fmt::Display> std::fmt::Display for ExecutableExpressionVisitError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structure(error) => error.fmt(f),
            Self::Visitor(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExecutableExpressionVisitError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Structure(error) => Some(error),
            Self::Visitor(error) => Some(error),
        }
    }
}
