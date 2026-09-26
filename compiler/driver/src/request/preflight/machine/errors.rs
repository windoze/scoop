use std::fmt;

#[derive(Debug)]
pub enum CurrentConeLirStageError {
    Identity(scoop_identity::IdentityValidationError),
    DiagnosticCatalog(scoop_identity::ExactTypeDiagnosticCatalogError),

    DependencyProjection(scoop_slib::CrossConeLirSelectionProjectionError),
    Lowering(scoop_lir_lower::StrongLirLoweringError),
    CrossConeBridge(scoop_lir_lower::CrossConeLirBridgeLoweringError),
}

impl fmt::Display for CurrentConeLirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(source) => source.fmt(formatter),
            Self::DiagnosticCatalog(source) => source.fmt(formatter),

            Self::DependencyProjection(source) => source.fmt(formatter),
            Self::Lowering(source) => source.fmt(formatter),
            Self::CrossConeBridge(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeLirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Identity(source) => source,
            Self::DiagnosticCatalog(source) => source,

            Self::DependencyProjection(source) => source,
            Self::Lowering(source) => source,
            Self::CrossConeBridge(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum CurrentConeMirStageError {
    DependencyProjection(scoop_slib::CrossConeMirSelectionProjectionError),
    Lowering(scoop_mir_lower::CurrentConeMirLoweringError),
    Foundation(scoop_mir::OdrFreeMirFoundationProjectionError),
    ProductionSection(scoop_mir_lower::MirProductionLoweringError),
    CrossConeBridge(scoop_mir_lower::CrossConeMirBridgeLoweringError),
    Sealing(scoop_mir::SingleConeStrongMirInputError),
}

impl fmt::Display for CurrentConeMirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DependencyProjection(source) => source.fmt(formatter),
            Self::Lowering(source) => source.fmt(formatter),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::CrossConeBridge(source) => source.fmt(formatter),
            Self::Sealing(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeMirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::DependencyProjection(source) => source,
            Self::Lowering(source) => source,
            Self::Foundation(source) => source,
            Self::ProductionSection(source) => source,
            Self::CrossConeBridge(source) => source,
            Self::Sealing(source) => source,
        })
    }
}
