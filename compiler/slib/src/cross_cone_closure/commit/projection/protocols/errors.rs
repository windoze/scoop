use scoop_hir::{CoreProtocolImportError, ImportedCoreProtocolCallableDefinition};
use scoop_identity::{ConeIdentity, PersistentFunctionId};

#[derive(Debug)]
pub enum CrossConeProtocolImportError {
    MissingProvider(ConeIdentity),
    MissingDefinitions(ConeIdentity),
    Import(CoreProtocolImportError),
}

impl std::fmt::Display for CrossConeProtocolImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProvider(provider) => write!(
                formatter,
                "compiler protocol provider {provider} is outside the dependency closure"
            ),
            Self::MissingDefinitions(provider) => write!(
                formatter,
                "provider {provider} has no compiler protocol definitions"
            ),
            Self::Import(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeProtocolImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Import(error) => Some(error),
            Self::MissingProvider(_) | Self::MissingDefinitions(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeInitializationSelectionError {
    ConsumerMismatch {
        closure: ConeIdentity,
        selected: ConeIdentity,
    },
    InvalidDefinition(ImportedCoreProtocolCallableDefinition),
    MissingProvider(ConeIdentity),
    SourceMismatch {
        provider: ConeIdentity,
        definition: PersistentFunctionId,
    },
    MissingMirBridge {
        provider: ConeIdentity,
        definition: PersistentFunctionId,
    },
    Projection(scoop_mir::ImportedMirCallableProjectionError),
    Selection(scoop_mir::SelectedExternalMirSetBuildError),
}

impl std::fmt::Display for CrossConeInitializationSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConsumerMismatch { closure, selected } => write!(
                formatter,
                "initialization selection consumer {selected} differs from closure {closure}"
            ),
            Self::InvalidDefinition(definition) => write!(
                formatter,
                "initialization service must be a source function, got {definition:?}"
            ),
            Self::MissingProvider(provider) => write!(
                formatter,
                "initialization service provider {provider} is outside the dependency closure"
            ),
            Self::SourceMismatch {
                provider,
                definition,
            } => write!(
                formatter,
                "initialization service {definition:?} disagrees with provider {provider}'s source declaration"
            ),
            Self::MissingMirBridge {
                provider,
                definition,
            } => write!(
                formatter,
                "provider {provider} has no MIR bridge for initialization service {definition:?}"
            ),
            Self::Projection(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeInitializationSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Projection(error) => Some(error),
            Self::Selection(error) => Some(error),
            Self::ConsumerMismatch { .. }
            | Self::InvalidDefinition(_)
            | Self::MissingProvider(_)
            | Self::SourceMismatch { .. }
            | Self::MissingMirBridge { .. } => None,
        }
    }
}
