use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyInterfaceSetBuildError {
    DuplicateDeclaration(PropertyDeclarationId),
}

impl fmt::Display for PropertyInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate property interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for PropertyInterfaceSetBuildError {}

#[derive(Debug)]
pub enum PropertyInterfaceSetValidationError<E> {
    SupportRecord {
        index: usize,
        error: PropertyInterfaceRecordResolutionError<E>,
    },
    Record {
        index: usize,
        error: PropertyInterfaceRecordResolutionError<E>,
    },
    DuplicateSupport(PropertyDeclarationId),
    DuplicateDeclaration {
        index: usize,
        declaration: PropertyDeclarationId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportRecord { index, error } => {
                write!(formatter, "invalid support property {index}: {error}")
            }
            Self::DuplicateSupport(declaration) => write!(
                formatter,
                "property {declaration:?} occurs in both public and support records"
            ),
            Self::Record { index, error } => {
                write!(formatter, "invalid property interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate property interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical property interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PropertyInterfaceSetValidationError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum PropertyInterfaceSetSemanticValidationError<E> {
    SupportRecord {
        index: usize,
        error: PropertyInterfaceSemanticValidationError<E>,
    },
    Record {
        index: usize,
        error: PropertyInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for PropertyInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportRecord { index, error } => write!(
                formatter,
                "invalid support property semantics {index}: {error}"
            ),
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid property interface semantics {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for PropertyInterfaceSetSemanticValidationError<E>
{
}
