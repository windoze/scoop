use crate::hir_interface_validation::CrossConeHirCallSiteOriginError;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum CrossConeHirTypeSiteError {
    Resource(WireError),
    Relations(Box<scoop_hir::HirDependencyTypeRelationError>),
    Origin(Box<CrossConeHirCallSiteOriginError>),
    Declaration(Box<scoop_hir::DeclarationTypeSiteValidationError>),
    Identity(scoop_identity::IdentityReferenceError),
    GeneratedType {
        position: scoop_hir::HirDependencyTypePositionV1,
        actual: scoop_identity::PersistentExactTypeId,
    },
}
impl From<WireError> for CrossConeHirTypeSiteError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for CrossConeHirTypeSiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Relations(error) => error.fmt(f),
            Self::Origin(error) => error.fmt(f),
            Self::Declaration(error) => error.fmt(f),
            Self::Identity(error) => error.fmt(f),
            Self::GeneratedType { position, actual } => {
                write!(f, "invalid generated type {actual} at {position:?}")
            }
        }
    }
}
impl std::error::Error for CrossConeHirTypeSiteError {}
