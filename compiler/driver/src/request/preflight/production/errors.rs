use std::fmt;

#[derive(Debug)]
pub enum CurrentConeProductionFailure {
    ArtifactOutputRequired,
    Dependencies(Box<crate::ExplicitDependencyValidationError>),
    Native(scoop_toolchain::ToolchainError),
    Hir(super::CurrentConeHirStageError),
    Mir(super::CurrentConeMirStageError),
    Lir(super::CurrentConeLirStageError),
    Warnings(crate::request::CurrentConeDiagnosticSetError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Layout(Box<super::LayoutProductionError>),
    Publication(scoop_slib::CrossConeArtifactPublishError),
    Documentation(super::DocumentationProductionError),
}

impl fmt::Display for CurrentConeProductionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactOutputRequired => {
                formatter.write_str("artifact production requires an artifact output")
            }
            Self::Dependencies(source) => source.fmt(formatter),
            Self::Native(source) => source.fmt(formatter),
            Self::Hir(source) => source.fmt(formatter),
            Self::Mir(source) => source.fmt(formatter),
            Self::Lir(source) => source.fmt(formatter),
            Self::Warnings(source) => source.fmt(formatter),
            Self::Producer(source) => source.fmt(formatter),
            Self::Cone(source) => source.fmt(formatter),
            Self::Layout(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
            Self::Documentation(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeProductionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ArtifactOutputRequired => return None,
            Self::Dependencies(source) => source.as_ref(),
            Self::Native(source) => source,
            Self::Hir(source) => source,
            Self::Mir(source) => source,
            Self::Lir(source) => source,
            Self::Warnings(source) => source,
            Self::Producer(source) => source,
            Self::Cone(source) => source,
            Self::Layout(source) => source.as_ref(),
            Self::Publication(source) => source,
            Self::Documentation(source) => source,
        })
    }
}

impl From<super::LayoutProductionError> for CurrentConeProductionFailure {
    fn from(error: super::LayoutProductionError) -> Self {
        Self::Layout(Box::new(error))
    }
}

/// Retains warnings and their source context after successful HIR lowering.
#[derive(Debug)]
pub struct CurrentConeProductionError {
    cause: Box<CurrentConeProductionFailure>,
    warnings: Option<crate::CurrentConeDiagnosticSet>,
}

impl CurrentConeProductionError {
    pub(in crate::request::preflight) fn before_hir(cause: CurrentConeProductionFailure) -> Self {
        Self {
            cause: Box::new(cause),
            warnings: None,
        }
    }

    pub(super) fn after_hir(
        cause: CurrentConeProductionFailure,
        warnings: crate::CurrentConeDiagnosticSet,
    ) -> Self {
        Self {
            cause: Box::new(cause),
            warnings: Some(warnings),
        }
    }

    pub fn cause(&self) -> &CurrentConeProductionFailure {
        &self.cause
    }

    pub const fn warnings(&self) -> Option<&crate::CurrentConeDiagnosticSet> {
        self.warnings.as_ref()
    }
}

impl fmt::Display for CurrentConeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(formatter)
    }
}

impl std::error::Error for CurrentConeProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
