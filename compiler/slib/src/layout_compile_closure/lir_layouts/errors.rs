use scoop_identity::ConeIdentity;
use scoop_lir as lir;

#[derive(Debug)]
pub enum SharedLirLayoutValidationError {
    Read(lir::LinkDataError),
    Resource(scoop_wire::WireError),
}

#[derive(Debug)]
pub struct CrossConeLayoutLirLayoutsError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirLayoutValidationError>,
}

impl CrossConeLayoutLirLayoutsError {
    pub(super) fn new(provider: ConeIdentity, source: SharedLirLayoutValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirLayoutValidationError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(lir::LinkDataError, Read);
from_error!(scoop_wire::WireError, Resource);

impl std::fmt::Display for SharedLirLayoutValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "saved LIR layout data: {self:?}")
    }
}
impl std::error::Error for SharedLirLayoutValidationError {}

impl std::fmt::Display for CrossConeLayoutLirLayoutsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid LIR layouts for {}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirLayoutsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
