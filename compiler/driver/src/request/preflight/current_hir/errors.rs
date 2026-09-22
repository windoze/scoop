use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeStrongProfileError {
    HirOdr(scoop_hir::OdrFreeHirFoundationError),
}

impl fmt::Display for CurrentConeStrongProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOdr(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeStrongProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirOdr(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CurrentConeHirStageError {
    SemanticWorld(scoop_hir::ImportedSemanticWorldBuildError),
    CoreInterface(scoop_hir::CoreProtocolImportError),
    CoreClassifier(scoop_hir::CoreClosedExactLeafClassifierBuildError),
    Input(scoop_hir_lower::CurrentConeSourceError),
    Lowering(Vec<scoop_ast::Diagnostic>),
    Foundation(scoop_hir::HirFoundationBuildError),
    ProductionSection(scoop_hir::CoreBootstrapInterfaceBuildError),
    CrossConeSection(
        Box<
            scoop_hir::CrossConeHirInterfaceProductionError<
                scoop_hir::CrossConeHirProductionAuthorityError,
            >,
        >,
    ),
}

impl fmt::Display for CurrentConeHirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticWorld(source) => source.fmt(formatter),
            Self::CoreInterface(source) => source.fmt(formatter),
            Self::CoreClassifier(source) => source.fmt(formatter),
            Self::Input(source) => source.fmt(formatter),
            Self::Lowering(diagnostics) => write!(
                formatter,
                "current Cone HIR lowering failed with {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::CrossConeSection(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticWorld(source) => Some(source),
            Self::CoreInterface(source) => Some(source),
            Self::CoreClassifier(source) => Some(source),
            Self::Input(source) => Some(source),
            Self::Lowering(_) => None,
            Self::Foundation(source) => Some(source),
            Self::ProductionSection(source) => Some(source),
            Self::CrossConeSection(source) => Some(source.as_ref()),
        }
    }
}
