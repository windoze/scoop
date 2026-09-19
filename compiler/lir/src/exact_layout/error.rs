#[derive(Debug)]
pub enum ExactLayoutReplayError {
    Identity(super::ExactLayoutIdentityError),
    Storage(crate::StorageReplayError),
    Shape(crate::TypeInstanceShapeError),
    Scan(crate::RefScanValidationError),
    Tuple(crate::TupleStorageReplayError),
    Enum(crate::EnumStorageGeometryErrorV1),
    Definition(crate::StrongShapeDefinitionError),
    Hash(scoop_wire::HashError),
    GeneratedNominal(scoop_identity::GeneratedNominalIdentityError),
    Resource(scoop_wire::WireError),
    IdentityKind,
    RepresentationRole,
    DependencyTarget,
    FieldOwner,
    VariantOwner,
    VariantFieldOwner,
    DuplicateVariant,
    DuplicateVariantField,
    MissingScan,
    MissingCLayout,
    ProviderMismatch,
    BaseKind,
    BoxPayloadIdentity,
    ArrayElementIdentity,
    EmptyEnum,
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactLayoutReplayError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(super::ExactLayoutIdentityError, Identity);
from_error!(crate::StorageReplayError, Storage);
from_error!(crate::TypeInstanceShapeError, Shape);
from_error!(crate::RefScanValidationError, Scan);
from_error!(crate::TupleStorageReplayError, Tuple);
from_error!(crate::EnumStorageGeometryErrorV1, Enum);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(scoop_wire::HashError, Hash);
from_error!(scoop_wire::WireError, Resource);

impl std::fmt::Display for ExactLayoutReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exact layout replay failed: {self:?}")
    }
}
impl std::error::Error for ExactLayoutReplayError {}
