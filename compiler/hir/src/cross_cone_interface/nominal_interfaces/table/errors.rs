use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalInterfaceSetBuildError {
    DuplicateDeclaration(SourceNominalId),
    NonPublicDeclaration(SourceNominalId),
    SupportLookup(SourceNominalId),
}

impl fmt::Display for NominalInterfaceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonPublicDeclaration(declaration) => write!(
                formatter,
                "non-public nominal {declaration:?} cannot enter public lookup"
            ),
            Self::SupportLookup(declaration) => write!(
                formatter,
                "support nominal {declaration:?} cannot carry public lookup relationships"
            ),
            Self::DuplicateDeclaration(declaration) => {
                write!(formatter, "duplicate nominal interface {declaration:?}")
            }
        }
    }
}

impl std::error::Error for NominalInterfaceSetBuildError {}

#[derive(Debug)]
pub enum NominalInterfaceSetValidationError<E> {
    Partition(NominalInterfaceSetBuildError),
    Record {
        index: usize,
        error: NominalInterfaceRecordResolutionError<E>,
    },
    DuplicateDeclaration {
        index: usize,
        declaration: SourceNominalId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for NominalInterfaceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Partition(error) => error.fmt(formatter),
            Self::Record { index, error } => {
                write!(formatter, "invalid nominal interface {index}: {error}")
            }
            Self::DuplicateDeclaration { index, declaration } => write!(
                formatter,
                "duplicate nominal interface {declaration:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => {
                write!(
                    formatter,
                    "non-canonical nominal interface order at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalInterfaceSetValidationError<E> {}

#[derive(Debug)]
pub enum NominalInterfaceSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: crate::NominalInterfaceSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for NominalInterfaceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => write!(
                formatter,
                "invalid nominal interface semantics at index {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for NominalInterfaceSetSemanticValidationError<E>
{
}
