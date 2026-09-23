use scoop_hir::{
    ExportDefaultLocalDataFlowValidationError, ExportDefaultTemplateKeyV1, SourceNominalId,
};
use scoop_identity::PersistentFieldId;
use scoop_wire::WireError;

#[derive(Debug)]
pub enum CrossConeHirDefaultDataFlowError {
    Resource(WireError),
    DuplicateField(PersistentFieldId),
    Template {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        source: Box<ExportDefaultLocalDataFlowValidationError<CrossConeHirDefaultFieldError>>,
    },
}

impl From<WireError> for CrossConeHirDefaultDataFlowError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for CrossConeHirDefaultDataFlowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::DuplicateField(field) => {
                write!(formatter, "duplicate default binding field {field}")
            }
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
            Self::Resource(source) => Some(source),
            Self::Template { source, .. } => Some(source.as_ref()),
            Self::DuplicateField(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeHirDefaultFieldError {
    MissingField(PersistentFieldId),
    NonNominalOwner(PersistentFieldId),
    Owner {
        declaration: PersistentFieldId,
        expected: SourceNominalId,
        actual: SourceNominalId,
    },
    Arity {
        declaration: PersistentFieldId,
        expected: u32,
        actual: u64,
    },
}

impl std::fmt::Display for CrossConeHirDefaultFieldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid default binding struct field: {self:?}")
    }
}

impl std::error::Error for CrossConeHirDefaultFieldError {}
