use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultCallableRefBuildError {
    TooManyTypeArguments,
}

impl fmt::Display for DefaultCallableRefBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyTypeArguments => {
                formatter.write_str("default callable type argument count exceeds u32")
            }
        }
    }
}

impl std::error::Error for DefaultCallableRefBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultCallableRefResolutionError<E> {
    Requirement(Box<crate::RequirementBindingResolutionError<E>>),
    Declaration(E),
    Owner(E),
    TooManyTypeArguments,
    TypeArgument { index: usize, error: E },
}

impl<E: fmt::Display> fmt::Display for DefaultCallableRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requirement(error) => write!(formatter, "{error}"),
            Self::Declaration(error) => {
                write!(formatter, "invalid default callable declaration: {error}")
            }
            Self::Owner(error) => write!(formatter, "invalid default callable owner: {error}"),
            Self::TooManyTypeArguments => {
                formatter.write_str("default callable type argument count exceeds u32")
            }
            Self::TypeArgument { index, error } => {
                write!(
                    formatter,
                    "invalid default callable type argument {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultCallableRefResolutionError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBoundCallableSourceResolutionError<E> {
    Bound(E),
    Callable(DefaultCallableRefResolutionError<E>),
    Member(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBoundCallableSourceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(error) => write!(formatter, "invalid bound callable constraint: {error}"),
            Self::Callable(error) => write!(formatter, "invalid bound class callable: {error}"),
            Self::Member(error) => write!(formatter, "invalid bound interface member: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBoundCallableSourceResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBoundCallableRefResolutionError<E> {
    ReceiverType(E),
    Source(DefaultBoundCallableSourceResolutionError<E>),
    InstantiatedSignature(E),
}

impl<E: fmt::Display> fmt::Display for DefaultBoundCallableRefResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReceiverType(error) => {
                write!(formatter, "invalid bound callable receiver type: {error}")
            }
            Self::Source(error) => write!(formatter, "invalid bound callable source: {error}"),
            Self::InstantiatedSignature(error) => {
                write!(formatter, "invalid bound callable signature: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBoundCallableRefResolutionError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultMethodCalleeResolutionError<E> {
    Callable(DefaultCallableRefResolutionError<E>),
    Bound(DefaultBoundCallableRefResolutionError<E>),
    DerivedEqualityOwner(E),
}

impl<E: fmt::Display> fmt::Display for DefaultMethodCalleeResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Callable(error) => write!(formatter, "invalid method callable: {error}"),
            Self::Bound(error) => write!(formatter, "invalid bound method callable: {error}"),
            Self::DerivedEqualityOwner(error) => {
                write!(formatter, "invalid derived equality owner: {error}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultMethodCalleeResolutionError<E> {}
