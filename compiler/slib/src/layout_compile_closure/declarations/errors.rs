use scoop_hir::CrossConeHirInternalClosureValidationError;
use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

use crate::{
    CrossConeHirCallableSurfaceError, CrossConeHirConstSurfaceError,
    CrossConeHirDefinitionSourceSurfaceError, CrossConeHirNominalSurfaceError,
    CrossConeHirPropertySurfaceError, CrossConeHirSourceInterfaceSurfaceError,
    CrossConeHirTypeAliasSurfaceError, hir_interface_validation::CrossConeHirReferenceSurfaceError,
};

#[derive(Debug)]
pub struct CrossConeLayoutHirDeclarationError {
    pub provider: ConeIdentity,
    pub source: Box<CrossConeHirDeclarationValidationError>,
}

#[derive(Debug)]
pub enum CrossConeHirDeclarationValidationError {
    Allocation { requested_slots: usize },
    Resource(WireError),
    Internal(Box<CrossConeHirInternalClosureValidationError>),
    DefinitionSources(CrossConeHirDefinitionSourceSurfaceError),
    Nominals(CrossConeHirNominalSurfaceError),
    Properties(CrossConeHirPropertySurfaceError),
    Callables(CrossConeHirCallableSurfaceError),
    TypeAliases(CrossConeHirTypeAliasSurfaceError),
    Sources(CrossConeHirSourceInterfaceSurfaceError),
    Constants(CrossConeHirConstSurfaceError),
    References(CrossConeHirReferenceSurfaceError),
}

impl std::fmt::Display for CrossConeLayoutHirDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid HIR declarations for {}: {}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutHirDeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl std::fmt::Display for CrossConeHirDeclarationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allocation { requested_slots } => {
                write!(
                    f,
                    "cannot allocate {requested_slots} HIR declaration provider slots"
                )
            }
            Self::Resource(error) => error.fmt(f),
            Self::Internal(error) => error.fmt(f),
            Self::DefinitionSources(error) => error.fmt(f),
            Self::Nominals(error) => error.fmt(f),
            Self::Properties(error) => error.fmt(f),
            Self::Callables(error) => error.fmt(f),
            Self::TypeAliases(error) => error.fmt(f),
            Self::Sources(error) => error.fmt(f),
            Self::Constants(error) => error.fmt(f),
            Self::References(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CrossConeHirDeclarationValidationError {}
