use scoop_hir::CoreProtocolImportError;
use scoop_identity::ConeIdentity;

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
