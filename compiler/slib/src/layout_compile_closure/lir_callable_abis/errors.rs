use scoop_identity::{ConeIdentity, StrongCallableDefinitionOwner};
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirCallableAbiValidationError {
    LocalProvider,
    LocalTarget,
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    Callable {
        target: StrongCallableDefinitionOwner,
        source: Box<lir::ExactCallableAbiError>,
    },
    Table(lir::ExactCallableAbiTableError),
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
