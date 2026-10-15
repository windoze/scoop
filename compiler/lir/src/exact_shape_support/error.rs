use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentTypeId};
use scoop_wire::{HashError, WireError};

#[derive(Debug)]
pub enum ParamFreeShapeSupportExportError {
    Provider,
    Target,
    ForeignSource {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    NonNominalSource,
    GenericSource,
    SourceIdentity(scoop_identity::SourceDeclarationIdentityError),
    GeneratedIdentity(scoop_identity::GeneratedNominalIdentityError),
    VariantIdentity(scoop_identity::EnumVariantIdentityError),
    Hash(HashError),
    MissingDescriptor(PersistentExactTypeId),
    MissingValueLayout(PersistentExactTypeId),
    ExactIdentity(PersistentExactTypeId),
    DescriptorLayout(PersistentExactTypeId),
    SourceRepresentation(PersistentExactTypeId),
    BoxedRepresentation(PersistentExactTypeId),
    HelperRepresentation(PersistentExactTypeId),
    HelperVariants(PersistentExactTypeId),
    HelperPayload(PersistentExactTypeId),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ParamFreeShapeSupportExportError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from_error!(
    scoop_identity::SourceDeclarationIdentityError,
    SourceIdentity
);
from_error!(
    scoop_identity::GeneratedNominalIdentityError,
    GeneratedIdentity
);
from_error!(scoop_identity::EnumVariantIdentityError, VariantIdentity);
from_error!(HashError, Hash);
from_error!(WireError, Resource);

impl std::fmt::Display for ParamFreeShapeSupportExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid param-free shape support: {self:?}")
    }
}

impl std::error::Error for ParamFreeShapeSupportExportError {}

#[derive(Debug)]
pub enum ParamFreeShapeSupportTableError {
    DuplicateRequiredSource(PersistentTypeId),
    TableProvider,
    TableTarget,
    Record(ParamFreeShapeSupportExportError),
    Resource(WireError),
}

impl From<ParamFreeShapeSupportExportError> for ParamFreeShapeSupportTableError {
    fn from(error: ParamFreeShapeSupportExportError) -> Self {
        Self::Record(error)
    }
}

impl From<WireError> for ParamFreeShapeSupportTableError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ParamFreeShapeSupportTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid shape-support table: {self:?}")
    }
}

impl std::error::Error for ParamFreeShapeSupportTableError {}
