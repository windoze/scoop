use super::*;

#[derive(Debug)]
pub enum LirDependencyReadError {
    Resource(scoop_wire::WireError),
    Lir(Box<lir::LayoutAbiSectionError>),
}

impl From<scoop_wire::WireError> for LirDependencyReadError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}

impl From<lir::LayoutAbiSectionError> for LirDependencyReadError {
    fn from(value: lir::LayoutAbiSectionError) -> Self {
        Self::Lir(Box::new(value))
    }
}

#[derive(Debug)]
pub struct CrossConeLayoutLirDependenciesError {
    pub provider: ConeIdentity,
    pub source: Box<LirDependencyReadError>,
}

impl std::fmt::Display for CrossConeLayoutLirDependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid LIR dependency references for {:?}: {:?}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutLirDependenciesError {}
