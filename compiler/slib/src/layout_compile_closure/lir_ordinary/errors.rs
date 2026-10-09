use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_lir as lir;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum SharedOrdinaryLirBridgeValidationError {
    ArtifactProvider,
    CallableSignature(DependencyCallableDeclarationId),
    CallableInterface(DependencyCallableDeclarationId),
    SourceAbi(Box<crate::NativeBoundaryCompileError>),
    LayoutSignature {
        declaration: DependencyCallableDeclarationId,
        exact: scoop_identity::PersistentExactTypeId,
    },
    Layout(super::super::SharedLirCallableAbiValidationError),
    Abi {
        declaration: DependencyCallableDeclarationId,
        source: Box<lir::ExactCallableAbiError>,
    },
    Export {
        declaration: DependencyCallableDeclarationId,
        source: Box<lir::ParamFreeLirCallableBuildError>,
    },
    DependencyProvider(ConeIdentity),
    DependencySources,
    MissingSelectedProvider(ConeIdentity),
    MissingSelectedExport {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    SelectedMirMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Section(lir::CrossConeLirBridgeBuildError),
    Wire(lir::CrossConeLirBridgeValidationError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for SharedOrdinaryLirBridgeValidationError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(super::super::SharedLirCallableAbiValidationError, Layout);
from_error!(lir::CrossConeLirBridgeBuildError, Section);
from_error!(lir::CrossConeLirBridgeValidationError, Wire);
from_error!(WireError, Resource);

impl From<crate::NativeBoundaryCompileError> for SharedOrdinaryLirBridgeValidationError {
    fn from(source: crate::NativeBoundaryCompileError) -> Self {
        Self::SourceAbi(Box::new(source))
    }
}

impl std::fmt::Display for SharedOrdinaryLirBridgeValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared ordinary LIR bridge: {self:?}")
    }
}
impl std::error::Error for SharedOrdinaryLirBridgeValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutOrdinaryLirBridgeError {
    pub provider: ConeIdentity,
    pub source: Box<SharedOrdinaryLirBridgeValidationError>,
}
impl std::fmt::Display for CrossConeLayoutOrdinaryLirBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid ordinary LIR bridge for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutOrdinaryLirBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
