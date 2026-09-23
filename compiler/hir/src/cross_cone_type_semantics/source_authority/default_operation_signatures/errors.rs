use super::*;

#[derive(Debug)]
pub enum DefaultSourceCallableOperationError {
    Resource(WireError),
    Nominal(Box<DefaultSourceNominalOperationError>),
    Member(Box<NominalMemberBindingError>),
    Constructor(Box<NominalConstructorBindingError>),
    Protocol(Box<NominalParameterBindingError>),
    Target(Box<DefaultSourceTargetSubjectError>),
    Declaration(DefaultCallableDeclarationV1),
    MissingOwner(SourceNominalId),
    Owner {
        expected: SourceNominalId,
        actual: SourceNominalId,
    },
    ConstructorSubject(DefinitionOriginSubject),
    AdapterRequiredParameter {
        constructor: PersistentConstructorId,
        position: usize,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultSourceNominalOperationError> for Error {
    fn from(error: DefaultSourceNominalOperationError) -> Self {
        match error {
            DefaultSourceNominalOperationError::Resource(error) => Self::Resource(error),
            error => Self::Nominal(Box::new(error)),
        }
    }
}
impl From<NominalMemberBindingError> for Error {
    fn from(error: NominalMemberBindingError) -> Self {
        match error {
            NominalMemberBindingError::Resource(error) => Self::Resource(error),
            error => Self::Member(Box::new(error)),
        }
    }
}
impl From<NominalConstructorBindingError> for Error {
    fn from(error: NominalConstructorBindingError) -> Self {
        match error {
            NominalConstructorBindingError::Resource(error) => Self::Resource(error),
            error => Self::Constructor(Box::new(error)),
        }
    }
}
impl From<NominalParameterBindingError> for Error {
    fn from(error: NominalParameterBindingError) -> Self {
        match error {
            NominalParameterBindingError::Resource(error) => Self::Resource(error),
            error => Self::Protocol(Box::new(error)),
        }
    }
}
impl From<DefaultSourceTargetSubjectError> for Error {
    fn from(error: DefaultSourceTargetSubjectError) -> Self {
        match error {
            DefaultSourceTargetSubjectError::Resource(error) => Self::Resource(error),
            error => Self::Target(Box::new(error)),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Nominal(error) => error.fmt(f),
            Self::Member(error) => error.fmt(f),
            Self::Constructor(error) => error.fmt(f),
            Self::Protocol(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::Declaration(declaration) => write!(
                f,
                "default callable {declaration:?} is not a nominal source member"
            ),
            Self::MissingOwner(owner) => write!(
                f,
                "default member requires an explicit application of generic owner {owner:?}"
            ),
            Self::Owner { expected, actual } => write!(
                f,
                "default callable requires owner {expected:?}, got {actual:?}"
            ),
            Self::ConstructorSubject(subject) => write!(
                f,
                "default constructor resolves to non-constructor subject {subject:?}"
            ),
            Self::AdapterRequiredParameter {
                constructor,
                position,
            } => write!(
                f,
                "zero-argument adapter of constructor {constructor} cannot omit required parameter {position}"
            ),
        }
    }
}
impl std::error::Error for Error {}
