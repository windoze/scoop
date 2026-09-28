use scoop_identity::{
    CallableDefinitionOwner, ConeIdentity, PersistentDispatchTableId, PersistentExactTypeId,
};
use scoop_lir as lir;
use scoop_wire::{HashError, WireError};

#[derive(Debug)]
pub enum SharedLirDispatchValidationError {
    LocalProvider,
    LocalTarget,
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    MissingType(PersistentExactTypeId),
    MissingSchema(PersistentExactTypeId),
    FiniteInheritance(PersistentExactTypeId),
    MissingCallable(CallableDefinitionOwner),
    DuplicateCallable(CallableDefinitionOwner),
    MissingValueLayout(PersistentExactTypeId),
    DuplicateValueLayout(PersistentExactTypeId),
    Replay {
        table: PersistentDispatchTableId,
        source: Box<lir::ExactDispatchError>,
    },
    Table(lir::ExactDispatchTableError),
    Hash(HashError),
    Resource(WireError),
}

impl From<lir::ExactDispatchTableError> for SharedLirDispatchValidationError {
    fn from(source: lir::ExactDispatchTableError) -> Self {
        Self::Table(source)
    }
}
impl From<HashError> for SharedLirDispatchValidationError {
    fn from(source: HashError) -> Self {
        Self::Hash(source)
    }
}
impl From<WireError> for SharedLirDispatchValidationError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
impl std::fmt::Display for SharedLirDispatchValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared MIR/LIR dispatch relation: {self:?}")
    }
}
impl std::error::Error for SharedLirDispatchValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirDispatchError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirDispatchValidationError>,
}
impl CrossConeLayoutLirDispatchError {
    pub(super) fn new(provider: ConeIdentity, source: SharedLirDispatchValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutLirDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid dispatch exports for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirDispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
