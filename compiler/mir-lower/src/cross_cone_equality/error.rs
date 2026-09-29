use super::*;

#[derive(Debug)]
pub enum SourceMirEqualityProductionError {
    Resource(scoop_wire::WireError),
    Identity(scoop_identity::IdentityReferenceError),
    Bridge(mir::MirCallableBridgeError),
    MissingMirMaterialization(PersistentGeneratedCallableId),
    MissingSignature(PersistentGeneratedCallableId),
    SignatureMismatch(PersistentGeneratedCallableId),
    InvalidRole(PersistentGeneratedCallableId),
    InvalidMaterialization(PersistentGeneratedCallableId),
}
impl From<scoop_wire::WireError> for Error {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<scoop_identity::IdentityReferenceError> for Error {
    fn from(error: scoop_identity::IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}
impl From<mir::MirCallableBridgeError> for Error {
    fn from(error: mir::MirCallableBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot produce derived equality MIR bindings: {self:?}"
        )
    }
}
impl std::error::Error for Error {}
