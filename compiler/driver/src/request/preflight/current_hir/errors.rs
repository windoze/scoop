use std::fmt;

#[derive(Debug)]
pub enum CurrentConeHirStageError {
    SemanticWorld(scoop_hir::ImportedSemanticWorldBuildError),
    NominalClassifier(scoop_hir::NominalExactLeafClassifierBuildError),
    Input(scoop_hir_lower::CurrentConeSourceError),
    Lowering(crate::CurrentConeDiagnosticSet),
    Diagnostics(crate::CurrentConeDiagnosticSetError),
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
            Self::NominalClassifier(source) => source.fmt(formatter),
            Self::Input(source) => source.fmt(formatter),
            Self::Lowering(diagnostics) => {
                write!(
                    formatter,
                    "current Cone HIR lowering failed with {} diagnostic(s)",
                    diagnostics.diagnostics().len()
                )?;
                write!(formatter, "\n{}", diagnostics.render_human())
            }
            Self::Diagnostics(source) => source.fmt(formatter),
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
            Self::NominalClassifier(source) => Some(source),
            Self::Input(source) => Some(source),
            Self::Lowering(_) => None,
            Self::Diagnostics(source) => Some(source),
            Self::Foundation(source) => Some(source),
            Self::ProductionSection(source) => Some(source),
            Self::CrossConeSection(source) => Some(source.as_ref()),
        }
    }
}
