use scoop_identity::IdentityReferenceError;
use scoop_wire::WireError;

use super::*;

#[derive(Debug)]
pub enum HirDependencyTypeRelationError {
    Resource(WireError),
    Encoding,
    Target(crate::ExternalHirTargetV1),
    ConflictingPosition(Position),
    NominalClosure(Position),
    CallResult(ExecutableExpressionPosition),
    UnreachableNominal {
        provider: ConeIdentity,
        owner: NominalDeclarationOwner,
    },
    Exact(Box<crate::HirTypeSiteExactError<IdentityReferenceError>>),
    Identity(Box<IdentityReferenceError>),
}

impl From<WireError> for HirDependencyTypeRelationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for HirDependencyTypeRelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared HIR expression type sites: {self:?}")
    }
}
impl std::error::Error for HirDependencyTypeRelationError {}
