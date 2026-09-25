use super::*;

#[derive(Debug)]
pub enum DefaultOperationProtocolTypeError {
    Resource(WireError),
    Arity {
        origin: PersistentGenericTypeId,
        actual: usize,
    },
    Copy(DefaultTemplateTypeSubstitutionError),
}
impl DefaultOperationProtocolTypeError {
    pub(super) fn copy(error: DefaultTemplateTypeSubstitutionError) -> Self {
        match error {
            DefaultTemplateTypeSubstitutionError::Wire(error) => Self::Resource(error),
            error => Self::Copy(error),
        }
    }
}
impl From<WireError> for DefaultOperationProtocolTypeError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefaultOperationProtocolTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Copy(error) => error.fmt(f),
            Self::Arity { origin, actual } => write!(
                f,
                "default operation protocol type {origin} requires one argument, got {actual}"
            ),
        }
    }
}
impl std::error::Error for DefaultOperationProtocolTypeError {}
