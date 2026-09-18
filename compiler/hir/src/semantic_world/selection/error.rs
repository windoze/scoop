use std::fmt;

use scoop_identity::{CallableTemplateOrigin, ConeIdentity};

use super::super::{DirectImportedTargetMergeError, ImportedTarget};
use crate::CoreClosedCallableClassificationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionPlanBuildError {
    Classification(CoreClosedCallableClassificationError),
    DuplicateCallable(CallableTemplateOrigin),
    TooManyCallables { count: usize },
}

impl fmt::Display for ImportedDependencySelectionPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Classification(error) => error.fmt(formatter),
            Self::DuplicateCallable(declaration) => write!(
                formatter,
                "dependency semantic world contains duplicate callable {declaration:?}"
            ),
            Self::TooManyCallables { count } => write!(
                formatter,
                "dependency semantic world contains {count} callables, exceeding the u32 id domain"
            ),
        }
    }
}

impl std::error::Error for ImportedDependencySelectionPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Classification(error) => Some(error),
            Self::DuplicateCallable(_) | Self::TooManyCallables { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedDependencyCandidateError {
    NotCallable(ImportedTarget),
    ForeignWorld,
    MissingCallable(CallableTemplateOrigin),
    TerminalProviderMismatch {
        declaration: CallableTemplateOrigin,
        expected: ConeIdentity,
    },
}

impl fmt::Display for ImportedDependencyCandidateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCallable(target) => {
                write!(formatter, "imported target {target:?} is not a callable")
            }
            Self::ForeignWorld => {
                formatter.write_str("imported binding belongs to another semantic world")
            }
            Self::MissingCallable(declaration) => write!(
                formatter,
                "imported callable {declaration:?} is absent from the dependency selection catalog"
            ),
            Self::TerminalProviderMismatch {
                declaration,
                expected,
            } => write!(
                formatter,
                "imported callable {declaration:?} does not terminate at provider {expected}"
            ),
        }
    }
}

impl std::error::Error for ImportedDependencyCandidateError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportedDependencySelectionError {
    ForeignProjection,
    CapabilityUnavailable { target: ImportedTarget },
    RouteMerge(DirectImportedTargetMergeError),
}

impl fmt::Display for ImportedDependencySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignProjection => {
                formatter.write_str("dependency callable candidate belongs to another projection")
            }
            Self::CapabilityUnavailable { target } => write!(
                formatter,
                "imported callable {target:?} is not executable by the M23-5 core-closed bridge"
            ),
            Self::RouteMerge(error) => write!(
                formatter,
                "cannot merge selected dependency callable routes: {error}"
            ),
        }
    }
}

impl std::error::Error for ImportedDependencySelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RouteMerge(error) => Some(error),
            Self::ForeignProjection | Self::CapabilityUnavailable { .. } => None,
        }
    }
}
