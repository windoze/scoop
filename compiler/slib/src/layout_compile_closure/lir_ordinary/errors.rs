use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedOrdinaryLirBridgeValidationError {
    Wire(lir::CrossConeLirBridgeValidationError),
    Resource(WireError),
}
impl From<lir::CrossConeLirBridgeValidationError> for SharedOrdinaryLirBridgeValidationError {
    fn from(error: lir::CrossConeLirBridgeValidationError) -> Self {
        Self::Wire(error)
    }
}
impl From<WireError> for SharedOrdinaryLirBridgeValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for SharedOrdinaryLirBridgeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid saved ordinary bridge: {self:?}")
    }
}
impl std::error::Error for SharedOrdinaryLirBridgeValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutOrdinaryLirBridgeError {
    pub provider: ConeIdentity,
    pub source: Box<SharedOrdinaryLirBridgeValidationError>,
}
impl std::fmt::Display for CrossConeLayoutOrdinaryLirBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid ordinary LIR bridge for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutOrdinaryLirBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
