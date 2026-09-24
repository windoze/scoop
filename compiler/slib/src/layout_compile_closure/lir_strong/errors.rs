use scoop_identity::{
    ConeIdentity, PersistentCallableBodyId, PersistentInitializationUnitId, PersistentSymbolKey,
};
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedLirStrongProductionError {
    Provider(ConeIdentity),
    ExternalDefinition(PersistentSymbolKey),
    ExternalAbi(PersistentSymbolKey),
    CallableBody(PersistentCallableBodyId),
    InitializationDefinition(PersistentInitializationUnitId),
    ShapeSources(scoop_hir::PublicNominalShapeProjectionError),
    SourceIdentity(scoop_identity::IdentityReferenceError),
    ExternalBridges(lir::StrongExternalLirBridgeReconstructionError),
    ShapeDefinition(lir::StrongShapeDefinitionError),
    TypeDefinitions(lir::StrongTypeReferenceResolutionErrorV2),
    InitializationDefinitions(lir::InitializationDependencyResolutionError),
    Replay(lir::StrongProductionSectionValidationError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedLirStrongProductionError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(scoop_hir::PublicNominalShapeProjectionError, ShapeSources);
from_error!(scoop_identity::IdentityReferenceError, SourceIdentity);
from_error!(
    lir::StrongExternalLirBridgeReconstructionError,
    ExternalBridges
);
from_error!(lir::StrongShapeDefinitionError, ShapeDefinition);
from_error!(lir::StrongTypeReferenceResolutionErrorV2, TypeDefinitions);
from_error!(
    lir::InitializationDependencyResolutionError,
    InitializationDefinitions
);
from_error!(lir::StrongProductionSectionValidationError, Replay);

impl std::fmt::Display for SharedLirStrongProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared Strong V2 production: {self:?}")
    }
}
impl std::error::Error for SharedLirStrongProductionError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirStrongProductionError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirStrongProductionError>,
}
impl std::fmt::Display for CrossConeLayoutLirStrongProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Strong V2 production for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirStrongProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
