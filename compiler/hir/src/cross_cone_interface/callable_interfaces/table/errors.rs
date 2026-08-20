use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableInterfaceSetBuildError {
    DuplicateDeclaration(CallableDeclarationId),
}

impl fmt::Display for CallableInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate callable interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for CallableInterfaceSetBuildError {}

#[derive(Debug)]
pub enum CallableInterfaceSetValidationError<E> {
    SupportRecord {
        index: usize,
        error: CallableInterfaceRecordResolutionError<E>,
    },
    Record {
        index: usize,
        error: CallableInterfaceRecordResolutionError<E>,
    },
    DuplicateSupport(CallableDeclarationId),
    DuplicateDeclaration {
        index: usize,
        declaration: CallableDeclarationId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportRecord { index, error } => {
                write!(formatter, "invalid support callable {index}: {error}")
            }
            Self::DuplicateSupport(declaration) => write!(
                formatter,
                "callable {declaration:?} occurs in both public and support records"
            ),
            Self::Record { index, error } => {
                write!(formatter, "invalid callable interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate callable interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical callable interface order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableInterfaceSetValidationError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum CallableInterfaceSetSemanticValidationError<E> {
    SupportRecord {
        index: usize,
        error: CallableInterfaceSemanticValidationError<E>,
    },
    Record {
        index: usize,
        error: CallableInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for CallableInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportRecord { index, error } => write!(
                formatter,
                "invalid support callable semantics {index}: {error}"
            ),
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid callable interface semantics {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CallableInterfaceSetSemanticValidationError<E>
{
}
