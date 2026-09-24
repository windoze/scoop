use std::convert::Infallible;

use scoop_hir::{PublicNominalShapeProjectionError, SharedTypeMetadataError};
use scoop_identity::{ConeIdentity, PersistentExactTypeId, PersistentTypeId};
use scoop_mir::{MirShapeSupportError, MirTypeBridgeSectionError};
use scoop_wire::WireError;

use crate::CrossConeLayoutMirFrontValidationError;

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
    MirFront(CrossConeLayoutMirFrontValidationError),
    MirTransport(MirTypeBridgeSectionError<Infallible>),
    ShapeProjection(PublicNominalShapeProjectionError),
    Shape(MirShapeSupportError),
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

#[derive(Debug)]
pub struct CrossConeLayoutMirTypesError {
    pub provider: ConeIdentity,
    pub source: Box<SharedMirTypeValidationError>,
}

impl CrossConeLayoutMirTypesError {
    pub(super) fn new(provider: ConeIdentity, source: SharedMirTypeValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
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
from_error!(CrossConeLayoutMirFrontValidationError, MirFront);
from_error!(MirTypeBridgeSectionError<Infallible>, MirTransport);
from_error!(PublicNominalShapeProjectionError, ShapeProjection);
from_error!(MirShapeSupportError, Shape);

impl std::fmt::Display for SharedMirTypeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared HIR/MIR type agreement: {self:?}")
    }
}

impl std::error::Error for SharedMirTypeValidationError {}

impl std::fmt::Display for CrossConeLayoutMirTypesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid MIR types for {}: {}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutMirTypesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
