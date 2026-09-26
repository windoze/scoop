use super::*;

#[derive(Debug)]
pub enum SharedMirDependencyGraphError {
    CallSites(Box<crate::CrossConeMirClosureRelationError>),
    InputProvider {
        source: ConeIdentity,
        mir: ConeIdentity,
    },
    Resource(scoop_wire::WireError),
    TypeOccurrences(Box<scoop_hir::HirDependencyTypeRelationError>),
    InitializationOccurrences(Box<scoop_hir::HirInitializationUseError>),
    InitializationUse(Box<mir::MirObjectBridgeError>),
    InitializationUseInventory,
    MissingInitializationUnit(scoop_identity::PersistentInitializationUnitId),
    Mir(Box<mir::MirTypeBridgeSectionError>),
}

impl From<scoop_wire::WireError> for SharedMirDependencyGraphError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}

impl From<mir::MirTypeBridgeSectionError> for SharedMirDependencyGraphError {
    fn from(value: mir::MirTypeBridgeSectionError) -> Self {
        Self::Mir(Box::new(value))
    }
}

#[derive(Debug)]
pub struct CrossConeLayoutMirDependenciesError {
    pub provider: ConeIdentity,
    pub source: Box<SharedMirDependencyGraphError>,
}

impl std::fmt::Display for CrossConeLayoutMirDependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid shared MIR dependency graph for {:?}: {:?}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutMirDependenciesError {}
