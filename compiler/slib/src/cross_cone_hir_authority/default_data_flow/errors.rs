use scoop_hir::{ExportDefaultLocalDataFlowValidationError, ExportDefaultTemplateKeyV1};

#[derive(Debug)]
pub enum CrossConeHirDefaultDataFlowError {
    Template {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        source: Box<ExportDefaultLocalDataFlowValidationError>,
    },
}

impl std::fmt::Display for CrossConeHirDefaultDataFlowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Template { index, key, source } => write!(
                formatter,
                "invalid data flow in default template {index} {key:?}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeHirDefaultDataFlowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Template { source, .. } => Some(source.as_ref()),
        }
    }
}
