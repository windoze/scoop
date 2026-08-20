use scoop_identity::{
    CallableMaterialization, ConeIdentity, PersistentInitializationUnitId,
    PersistentPropertyAccessorId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirInitializationUseError {
    Resource(scoop_wire::WireError),
    Identity(scoop_identity::IdentityReferenceError),
    Calls(Box<crate::DependencyCallOccurrenceError>),
    MissingAccessor(PersistentPropertyAccessorId),
    DuplicateAccessor(PersistentPropertyAccessorId),
    DuplicateUnit(PersistentPropertyAccessorId),
    MissingLocalUnit(PersistentInitializationUnitId),
    InitializationRootContext(CallableMaterialization),
    MissingProvider(ConeIdentity),
    DuplicateProvider(ConeIdentity),
    LocalProvider(ConeIdentity),
    NonAccessorUnit(scoop_identity::CallableTemplateOrigin),
}

impl From<scoop_wire::WireError> for HirInitializationUseError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<scoop_identity::IdentityReferenceError> for HirInitializationUseError {
    fn from(error: scoop_identity::IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}

impl std::fmt::Display for HirInitializationUseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid HIR initialization use: {self:?}")
    }
}

impl std::error::Error for HirInitializationUseError {}
