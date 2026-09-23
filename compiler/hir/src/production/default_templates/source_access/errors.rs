use crate::*;
use scoop_identity::PersistentGenericTypeId;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceAccessProductionError {
    Resource(scoop_wire::WireError),
    Owner(DefaultEntityProjectionError),
    MissingNominal(VisibilityOwner),
    MissingExact(ClassId),
    Domain(PersistentAccessDomainError),
    GenericSubclasses(CanonicalPersistentIdSetBuildError<PersistentGenericTypeId>),
    Encoding(scoop_wire::cbor::EncodeError),
    Build(DefaultSourceAccessBuildError),
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
            Self::MissingExact(owner) => write!(
                f,
                "default access class {owner:?} has no source exact identity"
            ),
            Self::Domain(error) => error.fmt(f),
            Self::GenericSubclasses(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
            Self::SharedBuild(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DefaultSourceAccessProductionError {}
