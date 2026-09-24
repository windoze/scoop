use scoop_identity::ConeIdentity;
use scoop_lir as lir;
use scoop_wire::WireError;
use std::convert::Infallible;

#[derive(Debug)]
pub enum SharedLirShapeSupportValidationError {
    MirProvider,
    Identity(scoop_identity::IdentityReferenceError),
    Replay(lir::ParamFreeShapeSupportTableError),
    Section(lir::LayoutAbiSectionError<Infallible>),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirShapeSupportValidationError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(scoop_identity::IdentityReferenceError, Identity);
from_error!(lir::ParamFreeShapeSupportTableError, Replay);
from_error!(lir::LayoutAbiSectionError<Infallible>, Section);
from_error!(WireError, Resource);

impl std::fmt::Display for SharedLirShapeSupportValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared MIR/LIR shape-support relation: {self:?}")
    }
}
impl std::error::Error for SharedLirShapeSupportValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirShapeSupportError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirShapeSupportValidationError>,
}
impl std::fmt::Display for CrossConeLayoutLirShapeSupportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid shape support for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirShapeSupportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
