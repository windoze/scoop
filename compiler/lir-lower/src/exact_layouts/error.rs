use super::*;

#[derive(Debug)]
pub enum ExactLayoutLoweringError {
    Provider,
    Target,
    Role(PersistentLayoutId),
    Cycle(PersistentLayoutId),
    MissingDependency(PersistentLayoutId),
    AmbiguousDependency(PersistentLayoutId),
    DependencyKind(PersistentExactTypeId),
    MissingMirShape(PersistentExactTypeId),
    MissingPhysicalType(PersistentExactTypeId),
    MissingSourceExact,
    SourceRepresentation(PersistentExactTypeId),
    SourceFields(PersistentExactTypeId),
    SourceBase(PersistentExactTypeId),
    SourceObject(PersistentExactTypeId),
    SourceFacts(PersistentExactTypeId),
    PhysicalLayout(PersistentLayoutId),
    PhysicalDescriptor(PersistentExactTypeId),
    MissingCLayout(PersistentExactTypeId),
    Identity(scoop_identity::IdentityReferenceError),
    Hash(scoop_wire::HashError),
    LayoutIdentity(lir::ExactLayoutIdentityError),
    Replay(lir::ExactLayoutReplayError),
    Table(lir::ExactLayoutTableError),
    Resource(scoop_wire::WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactLayoutLoweringError {
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
impl std::fmt::Display for ExactLayoutLoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exact layout projection failed: {self:?}")
    }
}
impl std::error::Error for ExactLayoutLoweringError {}
