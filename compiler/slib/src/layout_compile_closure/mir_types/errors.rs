use scoop_hir::{PublicNominalShapeProjectionError, SharedTypeMetadataError};
use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentTypeId};
use scoop_mir::MirShapeSupportError;
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirTypeComponent {
    Origin,
    Facts,
    Representation,
    FieldCount,
    Field { index: usize },
    VariantCount,
    Variant { index: usize },
    VariantField { variant: usize, index: usize },
    CLayout,
    InteriorMutable,
    ClassKind,
    Base,
    Interfaces,
    ObjectBacking,
}

#[derive(Debug)]
pub enum SharedMirTypeValidationError {
    Resource(WireError),
    Shared(SharedTypeMetadataError),
    ShapeProjection(PublicNominalShapeProjectionError),
    Shape(MirShapeSupportError),
    Type(scoop_mir::MirTypeBridgeError),
    ShapeProvider {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    MissingType(PersistentExactTypeId),
    UnexpectedType(PersistentExactTypeId),
    MissingFacts(PersistentExactTypeId),
    MissingInheritance(PersistentExactTypeId),
    MissingDeclaration(PersistentTypeId),
    Mismatch {
        exact: PersistentExactTypeId,
        component: SharedMirTypeComponent,
    },
}

impl SharedMirTypeValidationError {
    pub(super) fn require(
        exact: PersistentExactTypeId,
        component: SharedMirTypeComponent,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Mismatch { exact, component })
        }
    }
}

macro_rules! from_error {
    ($error:ty, $variant:ident) => {
        impl From<$error> for SharedMirTypeValidationError {
            fn from(error: $error) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(SharedTypeMetadataError, Shared);
from_error!(PublicNominalShapeProjectionError, ShapeProjection);
from_error!(MirShapeSupportError, Shape);
from_error!(scoop_mir::MirTypeBridgeError, Type);

impl std::fmt::Display for SharedMirTypeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared HIR/MIR type agreement: {self:?}")
    }
}

impl std::error::Error for SharedMirTypeValidationError {}
