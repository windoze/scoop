use super::*;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceAccessBindingError {
    Resource(WireError),
    Foundation(Box<TypeFoundationBindingError>),
    InvalidSubject(Subject),
    MissingKey(Subject),
    MissingRecord(Subject),
    UnexpectedRecord(Subject),
    ForeignKey(Subject),
    NonNominalOwner(Subject),
    Origin(Subject),
    NominalOverlap(SourceNominalId),
    Encoding(String),
    Access {
        subject: Subject,
        error: Box<DeclarationAccessSourceSemanticError<Self>>,
    },
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<TypeFoundationBindingError> for Error {
    fn from(error: TypeFoundationBindingError) -> Self {
        match error {
            TypeFoundationBindingError::Resource(error) => Self::Resource(error),
            other => Self::Foundation(Box::new(other)),
        }
    }
}
impl Error {
    pub(super) fn access(
        subject: Subject,
        error: DeclarationAccessSourceSemanticError<Self>,
    ) -> Self {
        match error {
            DeclarationAccessSourceSemanticError::Foundation(error)
            | DeclarationAccessSourceSemanticError::Origin(
                ExportDefinitionSourceSemanticValidationError::Foundation(error),
            ) => error,
            other => Self::Access {
                subject,
                error: Box::new(other),
            },
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::InvalidSubject(id) => write!(f, "invalid default access subject {id:?}"),
            Self::MissingKey(id) => {
                write!(f, "artifact has no default access declaration key {id:?}")
            }
            Self::MissingRecord(id) => write!(f, "missing default access source {id:?}"),
            Self::UnexpectedRecord(id) => write!(f, "unexpected default access source {id:?}"),
            Self::ForeignKey(id) => write!(
                f,
                "default access declaration {id:?} belongs to another provider"
            ),
            Self::NonNominalOwner(id) => write!(
                f,
                "default access declaration {id:?} has a non-nominal lexical owner"
            ),
            Self::Origin(id) => write!(
                f,
                "default access source {id:?} disagrees with the artifact definition origin"
            ),
            Self::NominalOverlap(id) => write!(
                f,
                "default access source disagrees with bound nominal {id:?}"
            ),
            Self::Encoding(reason) => write!(f, "cannot encode default access source: {reason}"),
            Self::Access { subject, error } => {
                write!(f, "default access source {subject:?}: {error}")
            }
        }
    }
}
impl std::error::Error for Error {}
