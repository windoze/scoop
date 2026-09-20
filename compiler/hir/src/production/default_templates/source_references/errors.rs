use crate::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum DefaultSourceReferencesProductionError {
    Resource(WireError),
    Entity(DefaultEntityProjectionError),
    DefinitionOrigin(HirDefinitionSourceProjectionError),
    Access(DefaultSourceAccessProductionError),
    Build(DefaultSourceReferencesBuildError),
    Provider {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
}
impl From<WireError> for DefaultSourceReferencesProductionError {
    fn from(e: WireError) -> Self {
        Self::Resource(e)
    }
}
impl From<DefaultEntityProjectionError> for DefaultSourceReferencesProductionError {
    fn from(e: DefaultEntityProjectionError) -> Self {
        Self::Entity(e)
    }
}
impl fmt::Display for DefaultSourceReferencesProductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Entity(e) => e.fmt(f),
            Self::DefinitionOrigin(e) => e.fmt(f),
            Self::Access(e) => e.fmt(f),
            Self::Build(e) => e.fmt(f),
            Self::Provider {
                kind,
                index,
                expected,
                actual,
            } => write!(
                f,
                "source {kind} occurrence {index} provider {actual:?} differs from body provider {expected:?}"
            ),
        }
    }
}
impl std::error::Error for DefaultSourceReferencesProductionError {}
impl DefaultSourceReferencesProductionError {
    pub fn resource_error(&self) -> Option<&WireError> {
        match self {
            Self::Resource(e) | Self::Entity(DefaultEntityProjectionError::Resource(e)) => Some(e),
            Self::Access(DefaultSourceAccessProductionError::Resource(e))
            | Self::Access(DefaultSourceAccessProductionError::Owner(
                DefaultEntityProjectionError::Resource(e),
            )) => Some(e),
            _ => None,
        }
    }
}
