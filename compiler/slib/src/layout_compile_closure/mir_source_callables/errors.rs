use std::convert::Infallible;

use scoop_hir as hir;
use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_mir::MirTypeBridgeSectionError;
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirSourceCallablePartition {
    Ordinary,
    TypeBridge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirSourceCallableComponent {
    Implementation,
    Receiver,
    ParameterCount,
    Parameter { index: usize },
    Result,
    Execution,
    GcEffect,
    LoweredSignature,
    LoweringRole,
    TrapSlot,
    Partition,
}

#[derive(Debug)]
pub enum SharedMirSourceCallableValidationError {
    Resource(WireError),
    Constructors(Box<super::SharedMirConstructorValidationError>),
    Objects(Box<super::SharedMirObjectValidationError>),
    Dispatch(Box<crate::SharedMirDispatchValidationError>),
    Equality(Box<super::SharedMirEqualityValidationError>),
    Shared(Box<hir::SharedTypeMetadataError>),
    MirTransport(Box<MirTypeBridgeSectionError<Infallible>>),
    Classifier(hir::NominalExactLeafClassifierBuildError),
    Encoding(scoop_wire::cbor::EncodeError),
    Provider {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Missing {
        declaration: DependencyCallableDeclarationId,
        partition: SharedMirSourceCallablePartition,
    },
    Unexpected {
        declaration: DependencyCallableDeclarationId,
        partition: SharedMirSourceCallablePartition,
    },
    Mismatch {
        declaration: DependencyCallableDeclarationId,
        component: SharedMirSourceCallableComponent,
    },
}

impl SharedMirSourceCallableValidationError {
    pub(super) fn require(
        declaration: DependencyCallableDeclarationId,
        component: SharedMirSourceCallableComponent,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Mismatch {
                declaration,
                component,
            })
        }
    }
}

impl From<WireError> for SharedMirSourceCallableValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<hir::SharedTypeMetadataError> for SharedMirSourceCallableValidationError {
    fn from(error: hir::SharedTypeMetadataError) -> Self {
        Self::Shared(Box::new(error))
    }
}
impl From<MirTypeBridgeSectionError<Infallible>> for SharedMirSourceCallableValidationError {
    fn from(error: MirTypeBridgeSectionError<Infallible>) -> Self {
        Self::MirTransport(Box::new(error))
    }
}

impl std::fmt::Display for SharedMirSourceCallableValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared HIR/MIR source callable agreement: {self:?}")
    }
}
impl std::error::Error for SharedMirSourceCallableValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutMirSourceCallablesError {
    pub provider: ConeIdentity,
    pub source: Box<SharedMirSourceCallableValidationError>,
}

impl CrossConeLayoutMirSourceCallablesError {
    pub(super) fn new(
        provider: ConeIdentity,
        source: SharedMirSourceCallableValidationError,
    ) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutMirSourceCallablesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid MIR source callables for {}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutMirSourceCallablesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
