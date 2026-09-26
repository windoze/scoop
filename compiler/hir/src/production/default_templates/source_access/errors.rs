use crate::*;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceAccessProductionError {
    Resource(scoop_wire::WireError),
    Owner(DefaultEntityProjectionError),
    MissingNominal(VisibilityOwner),
    SharedBuild(ExportDefaultAccessWitnessBuildError),
}
impl From<scoop_wire::WireError> for DefaultSourceAccessProductionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for DefaultSourceAccessProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Owner(error) => error.fmt(f),
            Self::MissingNominal(owner) => write!(
                f,
                "default access owner {owner:?} has no source nominal identity"
            ),
            Self::SharedBuild(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DefaultSourceAccessProductionError {}
