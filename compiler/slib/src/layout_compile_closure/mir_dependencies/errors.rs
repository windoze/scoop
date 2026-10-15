use super::*;

#[derive(Debug)]
pub enum MirDependencyReadError {
    Resource(scoop_wire::WireError),
    Mir(Box<mir::MirTypeBridgeSectionError>),
}

impl From<scoop_wire::WireError> for MirDependencyReadError {
    fn from(value: scoop_wire::WireError) -> Self {
        Self::Resource(value)
    }
}

impl From<mir::MirTypeBridgeSectionError> for MirDependencyReadError {
    fn from(value: mir::MirTypeBridgeSectionError) -> Self {
        Self::Mir(Box::new(value))
    }
}

#[derive(Debug)]
pub struct CrossConeLayoutMirDependenciesError {
    pub provider: ConeIdentity,
    pub source: Box<MirDependencyReadError>,
}

impl std::fmt::Display for CrossConeLayoutMirDependenciesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid MIR dependency references for {:?}: {:?}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutMirDependenciesError {}
