use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirInitializationAbiValidationError {
    InitializationTarget,
    InitializationSignature,
    Layouts(super::super::SharedLirCallableAbiValidationError),
    SignatureLayouts(lir::ExactCallableAbiError),
    Abi(lir::CallableAbiReplayError),
    Wire(lir::StrongInitializationAbiValidationError),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirInitializationAbiValidationError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(super::super::SharedLirCallableAbiValidationError, Layouts);
from_error!(lir::CallableAbiReplayError, Abi);
from_error!(lir::StrongInitializationAbiValidationError, Wire);
from_error!(WireError, Resource);
impl std::fmt::Display for SharedLirInitializationAbiValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared initialization ABI: {self:?}")
    }
}
impl std::error::Error for SharedLirInitializationAbiValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirInitializationAbiError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirInitializationAbiValidationError>,
}
impl std::fmt::Display for CrossConeLayoutLirInitializationAbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid initialization ABI for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirInitializationAbiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
