use super::*;
use std::fmt;

#[derive(Debug)]
pub enum ProtectedDeclarationBindingError {
    Resource(WireError),
    Source(Box<NominalNestedBindingError>),
    Inventory,
}
impl From<WireError> for ProtectedDeclarationBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<NominalNestedBindingError> for ProtectedDeclarationBindingError {
    fn from(error: NominalNestedBindingError) -> Self {
        match error {
            NominalNestedBindingError::Resource(e) => Self::Resource(e),
            other => Self::Source(Box::new(other)),
        }
    }
}
impl fmt::Display for ProtectedDeclarationBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Inventory => f.write_str(
                "protected declarations differ from the complete bound source inventory",
            ),
        }
    }
}
impl std::error::Error for ProtectedDeclarationBindingError {}
