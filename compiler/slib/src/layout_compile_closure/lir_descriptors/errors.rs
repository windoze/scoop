use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirDescriptorValidationError {
    LocalProvider,
    LocalTarget,
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    MissingDescriptor(PersistentExactTypeId),
    DuplicateDescriptor(PersistentExactTypeId),
    Replay {
        exact: PersistentExactTypeId,
        source: Box<lir::ExactDescriptorError>,
    },
    Table(lir::ExactDescriptorTableError),
    Diagnostic(scoop_identity::ExactTypeDiagnosticCatalogError),
    Encoding(scoop_wire::cbor::EncodeError),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirDescriptorValidationError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(lir::ExactDescriptorTableError, Table);
from_error!(scoop_identity::ExactTypeDiagnosticCatalogError, Diagnostic);
from_error!(scoop_wire::cbor::EncodeError, Encoding);
from_error!(WireError, Resource);

impl std::fmt::Display for SharedLirDescriptorValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared MIR/LIR descriptor relation: {self:?}")
    }
}
impl std::error::Error for SharedLirDescriptorValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirDescriptorsError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirDescriptorValidationError>,
}
impl CrossConeLayoutLirDescriptorsError {
    pub(super) fn new(provider: ConeIdentity, source: SharedLirDescriptorValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutLirDescriptorsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid descriptors for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirDescriptorsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
