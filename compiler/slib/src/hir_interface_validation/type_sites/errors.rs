use crate::hir_interface_validation::CrossConeHirCallSiteOriginError;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum CrossConeHirTypeSiteError {
    Resource(WireError),
    Relations(Box<scoop_hir::HirDependencyTypeRelationError>),
    Origin(Box<CrossConeHirCallSiteOriginError>),
    Declaration(Box<scoop_hir::DeclarationTypeSiteValidationError>),
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
        }
    }
}
impl std::error::Error for CrossConeHirTypeSiteError {}
