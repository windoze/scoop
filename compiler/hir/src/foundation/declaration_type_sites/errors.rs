use crate::{DefinitionSourceLocationValidationError, HirCallableTypePositionV1};
use scoop_identity::{CallableMaterialization, ConeIdentity, DefinitionOriginSubject};
use scoop_wire::WireError;

#[derive(Debug)]
pub enum DeclarationTypeSiteValidationError {
    Resource(WireError),
    ExpressionPosition,
    Materialization(CallableMaterialization),
    SignaturePosition(CallableMaterialization, HirCallableTypePositionV1),
    StoragePosition(crate::HirDependencyTypePositionV1),
    MissingIdentity {
        kind: &'static str,
        id: [u8; 32],
    },
    MissingOrigin(DefinitionOriginSubject),
    Provider {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Origin(Box<DefinitionSourceLocationValidationError>),
}

impl From<WireError> for DeclarationTypeSiteValidationError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
impl std::fmt::Display for DeclarationTypeSiteValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid HIR declaration type position: {self:?}")
    }
}
impl std::error::Error for DeclarationTypeSiteValidationError {}
