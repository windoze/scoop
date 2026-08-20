use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentLayoutId};
use scoop_lir as lir;

#[derive(Debug)]
pub enum SharedLirLayoutValidationError {
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    AmbiguousDependency(PersistentLayoutId),
    MissingDependency(PersistentLayoutId),
    DependencyKind(PersistentExactTypeId),
    MissingMirShape(PersistentExactTypeId),
    Cycle(PersistentLayoutId),
    Role(PersistentLayoutId),
    SourceObject(PersistentExactTypeId),
    SourceFacts(PersistentExactTypeId),
    CLayout(PersistentExactTypeId),
    ArithmeticOverflow,
    Identity(scoop_identity::IdentityReferenceError),
    Hash(scoop_wire::HashError),
    LayoutIdentity(lir::ExactLayoutIdentityError),
    Replay(lir::ExactLayoutReplayError),
    Table(lir::ExactLayoutTableError),
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
from_error!(scoop_identity::IdentityReferenceError, Identity);
from_error!(scoop_wire::HashError, Hash);
from_error!(lir::ExactLayoutIdentityError, LayoutIdentity);
from_error!(lir::ExactLayoutReplayError, Replay);
from_error!(lir::ExactLayoutTableError, Table);
from_error!(scoop_wire::WireError, Resource);

impl std::fmt::Display for SharedLirLayoutValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared MIR/LIR layout replay: {self:?}")
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
