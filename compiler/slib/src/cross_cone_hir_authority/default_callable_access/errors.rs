use scoop_hir::{
    DefaultSourceNestedCallableQueryError, DefaultSourceTargetSubjectError,
    ExportDefaultReferenceOccurrenceSiteV1, ExportDefaultTemplateKeyV1, SourceNominalId,
};
use scoop_wire::WireError;

use crate::cross_cone_hir_authority::{
    CrossConeHirDefaultTypeAccessError, CrossConeHirNominalAuthorityError,
};

#[derive(Debug)]
pub enum CrossConeHirDefaultCallableAccessError {
    Resource(WireError),
    Target(Box<DefaultSourceTargetSubjectError>),
    Nested(Box<DefaultSourceNestedCallableQueryError>),
    Declaration(Box<CrossConeHirNominalAuthorityError>),
    Type(Box<CrossConeHirDefaultTypeAccessError>),
    Encoding(String),
    EqualityShape,
    EqualityKind(SourceNominalId),
    WitnessDomain,
    ReferenceMatch {
        site: ExportDefaultReferenceOccurrenceSiteV1,
        reason: &'static str,
    },
    Occurrence {
        index: usize,
        site: ExportDefaultReferenceOccurrenceSiteV1,
        expression_index: Option<u32>,
        source: Box<Self>,
    },
    Template {
        key: ExportDefaultTemplateKeyV1,
        source: Box<Self>,
    },
}

type Error = CrossConeHirDefaultCallableAccessError;

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultSourceTargetSubjectError> for Error {
    fn from(error: DefaultSourceTargetSubjectError) -> Self {
        match error {
            DefaultSourceTargetSubjectError::Resource(error) => Self::Resource(error),
            error => Self::Target(Box::new(error)),
        }
    }
}
impl From<DefaultSourceNestedCallableQueryError> for Error {
    fn from(error: DefaultSourceNestedCallableQueryError) -> Self {
        match error {
            DefaultSourceNestedCallableQueryError::Resource(error) => Self::Resource(error),
            error => Self::Nested(Box::new(error)),
        }
    }
}
impl From<CrossConeHirNominalAuthorityError> for Error {
    fn from(error: CrossConeHirNominalAuthorityError) -> Self {
        match error {
            CrossConeHirNominalAuthorityError::Resource(error) => Self::Resource(error),
            error => Self::Declaration(Box::new(error)),
        }
    }
}
impl From<CrossConeHirDefaultTypeAccessError> for Error {
    fn from(error: CrossConeHirDefaultTypeAccessError) -> Self {
        match error {
            CrossConeHirDefaultTypeAccessError::Resource(error) => Self::Resource(error),
            error => Self::Type(Box::new(error)),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Target(error) => error.fmt(f),
            Self::Nested(error) => error.fmt(f),
            Self::Declaration(error) => error.fmt(f),
            Self::Type(error) => error.fmt(f),
            Self::Encoding(error) => write!(f, "cannot encode default callable access: {error}"),
            Self::EqualityShape => {
                f.write_str("default equality requires a nominal or structural equality source")
            }
            Self::EqualityKind(id) => {
                write!(f, "default equality source {id:?} is not a struct or enum")
            }
            Self::WitnessDomain => f.write_str(
                "default callable access witness differs from the actual source declaration domain",
            ),
            Self::ReferenceMatch { site, reason } => {
                write!(f, "default callable at {site:?}: {reason}")
            }
            Self::Occurrence {
                index,
                site,
                expression_index,
                source,
            } => write!(
                f,
                "callable reference[{index}] at {site:?}, expression {expression_index:?}, has invalid source access: {source}"
            ),
            Self::Template { key, source } => {
                write!(f, "default {key:?} has invalid callable access: {source}")
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Target(error) => Some(error.as_ref()),
            Self::Nested(error) => Some(error.as_ref()),
            Self::Declaration(error) => Some(error.as_ref()),
            Self::Type(error) => Some(error.as_ref()),
            Self::Template { source, .. } | Self::Occurrence { source, .. } => {
                Some(source.as_ref())
            }
            _ => None,
        }
    }
}
