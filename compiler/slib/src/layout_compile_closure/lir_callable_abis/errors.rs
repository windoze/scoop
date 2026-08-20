use scoop_identity::{CallableDefinitionOwner, ConeIdentity};
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirCallableAbiValidationError {
    LocalProvider,
    LocalTarget,
    SignatureMismatch(CallableDefinitionOwner),
    StorageMismatch(scoop_identity::PersistentExactTypeId),
    Read(lir::LinkDataError),
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    Callable {
        target: CallableDefinitionOwner,
        source: Box<lir::ExactCallableAbiError>,
    },
    Table(lir::ExactCallableAbiTableError),
    Abi(lir::ExactCallableAbiError),
    Signature(scoop_identity::ScoopAbiError),
    Identity(scoop_identity::IdentityReferenceError),
    Tuple(lir::TupleStorageReplayError),
    Storage(lir::StorageReplayError),
    Shape(lir::TypeInstanceShapeError),
    Resource(WireError),
}
impl From<lir::ExactCallableAbiTableError> for SharedLirCallableAbiValidationError {
    fn from(source: lir::ExactCallableAbiTableError) -> Self {
        Self::Table(source)
    }
}
impl From<WireError> for SharedLirCallableAbiValidationError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirCallableAbiValidationError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(lir::ExactCallableAbiError, Abi);
from_error!(lir::LinkDataError, Read);
from_error!(scoop_identity::ScoopAbiError, Signature);
from_error!(scoop_identity::IdentityReferenceError, Identity);
from_error!(lir::TupleStorageReplayError, Tuple);
from_error!(lir::StorageReplayError, Storage);
from_error!(lir::TypeInstanceShapeError, Shape);

impl std::fmt::Display for SharedLirCallableAbiValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared MIR/LIR callable ABI relation: {self:?}")
    }
}
impl std::error::Error for SharedLirCallableAbiValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirCallableAbisError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirCallableAbiValidationError>,
}
impl CrossConeLayoutLirCallableAbisError {
    pub(super) fn new(provider: ConeIdentity, source: SharedLirCallableAbiValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutLirCallableAbisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid callable ABIs for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirCallableAbisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
