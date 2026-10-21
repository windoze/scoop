#[derive(Debug)]
pub enum DocumentationProductionError {
    Projection(scoop_doc::ProjectionError),
    Identity(scoop_wire::HashError),
    Encoding(String),
    Diagnostics(crate::CurrentConeDiagnosticSetError),
    Publication {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for DocumentationProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Projection(source) => source.fmt(formatter),
            Self::Identity(source) => source.fmt(formatter),
            Self::Diagnostics(source) => source.fmt(formatter),
            Self::Encoding(message) => {
                write!(formatter, "cannot encode documentation input: {message}")
            }
            Self::Publication { path, source } => write!(
                formatter,
                "cannot publish documentation input {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DocumentationProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Projection(source) => Some(source),
            Self::Identity(source) => Some(source),
            Self::Diagnostics(source) => Some(source),
            Self::Encoding(_) => None,
            Self::Publication { source, .. } => Some(source),
        }
    }
}
