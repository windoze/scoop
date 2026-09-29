use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ImportedDefaultMaterializationError {
    MissingReceiver,
    UnknownLocal(LocalValueSelector),
    ExpectedMaterializedLocal(LocalValueSelector),
    InvalidControlFlow(&'static str),
    MissingCallable(scoop_identity::CallableTemplateOrigin),
    DependencySelection(String),
    DefinitionOrigin(ImportedDefinitionOriginError),
    Plan(String),
}

impl From<ImportedDefinitionOriginError> for ImportedDefaultMaterializationError {
    fn from(value: ImportedDefinitionOriginError) -> Self {
        Self::DefinitionOrigin(value)
    }
}

impl fmt::Display for ImportedDefaultMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingReceiver => {
                formatter.write_str("dependency default is missing its receiver value")
            }
            Self::UnknownLocal(local) => {
                write!(
                    formatter,
                    "dependency default reads unmapped local {local:?}"
                )
            }
            Self::ExpectedMaterializedLocal(local) => write!(
                formatter,
                "dependency default local {local:?} is not backed by a materialized local"
            ),
            Self::InvalidControlFlow(operation) => {
                write!(
                    formatter,
                    "invalid dependency default control flow: {operation}"
                )
            }
            Self::MissingCallable(callee) => {
                write!(
                    formatter,
                    "dependency default call {callee:?} has no declaration"
                )
            }
            Self::DependencySelection(error) => {
                write!(
                    formatter,
                    "dependency default callable selection failed: {error}"
                )
            }
            Self::DefinitionOrigin(error) => error.fmt(formatter),
            Self::Plan(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for ImportedDefaultMaterializationError {}
