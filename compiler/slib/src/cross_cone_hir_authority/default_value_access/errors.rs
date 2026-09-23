use scoop_hir::{
    DefaultSourceTargetSubjectError, ExportDefaultReferenceKindV1, ExportDefaultTemplateKeyV1,
};
use scoop_wire::WireError;

use crate::cross_cone_hir_authority::CrossConeHirNominalAuthorityError;

#[derive(Debug)]
pub enum CrossConeHirDefaultValueAccessError {
    Resource(WireError),
    Target(Box<DefaultSourceTargetSubjectError>),
    Declaration(Box<CrossConeHirNominalAuthorityError>),
    Encoding(String),
    WitnessDomain,
    Reference {
        template: ExportDefaultTemplateKeyV1,
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        source: Box<Self>,
    },
}

impl From<WireError> for CrossConeHirDefaultValueAccessError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultSourceTargetSubjectError> for CrossConeHirDefaultValueAccessError {
    fn from(error: DefaultSourceTargetSubjectError) -> Self {
        match error {
            DefaultSourceTargetSubjectError::Resource(error) => Self::Resource(error),
            error => Self::Target(Box::new(error)),
        }
    }
}
impl From<CrossConeHirNominalAuthorityError> for CrossConeHirDefaultValueAccessError {
    fn from(error: CrossConeHirNominalAuthorityError) -> Self {
        match error {
            CrossConeHirNominalAuthorityError::Resource(error) => Self::Resource(error),
            error => Self::Declaration(Box::new(error)),
        }
    }
}
impl std::fmt::Display for CrossConeHirDefaultValueAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::Declaration(error) => error.fmt(f),
            Self::Encoding(error) => {
                write!(f, "cannot encode default value access domain: {error}")
            }
            Self::WitnessDomain => f.write_str(
                "default value access witness differs from the actual source declaration domain",
            ),
            Self::Reference {
                template,
                kind,
                index,
                source,
            } => write!(
                f,
                "default {template:?} {kind} reference[{index}] has invalid source access: {source}"
            ),
        }
    }
}
impl std::error::Error for CrossConeHirDefaultValueAccessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Target(error) => Some(error.as_ref()),
            Self::Declaration(error) => Some(error.as_ref()),
            Self::Reference { source, .. } => Some(source.as_ref()),
            Self::Encoding(_) | Self::WitnessDomain => None,
        }
    }
}
