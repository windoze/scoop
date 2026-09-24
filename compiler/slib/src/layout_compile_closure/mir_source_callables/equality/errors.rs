use super::*;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedMirEqualityValidationError {
    Resource(WireError),
    Shared(Box<hir::SharedTypeMetadataError>),
    Identity(scoop_identity::IdentityReferenceError),
    MissingSource(PersistentGeneratedCallableId),
    MissingCallable(PersistentGeneratedCallableId),
    UnexpectedCallable(PersistentGeneratedCallableId),
    Owner(PersistentExactTypeId),
    BooleanSource,
    Definition(PersistentGeneratedCallableId),
    Signature(PersistentGeneratedCallableId),
    Role(PersistentGeneratedCallableId),
}

impl From<WireError> for SharedMirEqualityValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<scoop_identity::IdentityReferenceError> for SharedMirEqualityValidationError {
    fn from(error: scoop_identity::IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}
impl From<hir::SharedTypeMetadataError> for SharedMirEqualityValidationError {
    fn from(error: hir::SharedTypeMetadataError) -> Self {
        match error {
            hir::SharedTypeMetadataError::Resource(error) => Self::Resource(error),
            error => Self::Shared(Box::new(error)),
        }
    }
}
impl std::fmt::Display for SharedMirEqualityValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "shared HIR/MIR derived equality agreement: {self:?}"
        )
    }
}
impl std::error::Error for SharedMirEqualityValidationError {}
