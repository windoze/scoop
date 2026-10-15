use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirCallableAbiValidationError {
    Read(lir::LinkDataError),
    Resource(WireError),
}
impl From<lir::LinkDataError> for SharedLirCallableAbiValidationError {
    fn from(error: lir::LinkDataError) -> Self {
        Self::Read(error)
    }
}
impl From<WireError> for SharedLirCallableAbiValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for SharedLirCallableAbiValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid saved callable ABI: {self:?}")
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
