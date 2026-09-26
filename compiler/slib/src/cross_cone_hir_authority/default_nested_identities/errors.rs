use scoop_hir::{
    DefaultNestedCallableSiteV1, DefaultNestedIdentityValidationError,
    DefaultSourceNestedCallableQueryError, ExportDefaultTemplateKeyV1,
};
use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum CrossConeHirDefaultNestedIdentityError {
    Resource(WireError),
    UnreachableProvider(ConeIdentity),
    Reference(DefaultSourceNestedCallableQueryError),
    Occurrence {
        site: DefaultNestedCallableSiteV1,
        source: Box<DefaultNestedIdentityValidationError>,
    },
    Template {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        source: Box<Self>,
    },
}

impl From<WireError> for CrossConeHirDefaultNestedIdentityError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for CrossConeHirDefaultNestedIdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Reference(error) => error.fmt(f),
            Self::UnreachableProvider(provider) => write!(
                f,
                "nested default identity has no reachable provider {provider}"
            ),
            Self::Occurrence { site, source } => {
                write!(f, "default nested occurrence {site:?}: {source}")
            }
            Self::Template { index, key, source } => write!(
                f,
                "default[{index}] {key:?} has invalid nested identity: {source}"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirDefaultNestedIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Reference(error) => Some(error),
            Self::Occurrence { source, .. } => Some(source.as_ref()),
            Self::Template { source, .. } => Some(source.as_ref()),
            Self::UnreachableProvider(_) => None,
        }
    }
}
