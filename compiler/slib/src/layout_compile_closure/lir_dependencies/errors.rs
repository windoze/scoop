use super::*;

#[derive(Debug)]
pub enum SharedLirDependencyGraphError {
    InputProvider {
        source: ConeIdentity,
        layout: ConeIdentity,
    },
    DependencyProvider(ConeIdentity),
    DependencyTarget(ConeIdentity),
    MissingTypeLayout {
        provider: ConeIdentity,
        exact: scoop_identity::PersistentExactTypeId,
    },
    Resource(scoop_wire::WireError),
    TypeOccurrences(Box<scoop_hir::HirDependencyTypeRelationError>),
    InitializationEdges(Box<mir::MirObjectBridgeError>),
    Lir(Box<lir::LayoutAbiSectionError>),
}

impl From<scoop_wire::WireError> for SharedLirDependencyGraphError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}

impl From<lir::LayoutAbiSectionError> for SharedLirDependencyGraphError {
    fn from(value: lir::LayoutAbiSectionError) -> Self {
        Self::Lir(Box::new(value))
    }
}

#[derive(Debug)]
pub struct CrossConeLayoutLirDependenciesError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirDependencyGraphError>,
}

impl std::fmt::Display for CrossConeLayoutLirDependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid shared LIR dependency graph for {:?}: {:?}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutLirDependenciesError {}
