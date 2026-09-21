use super::super::{CurrentConeLirStageError, CurrentConeMirStageError};
use super::*;

#[derive(Debug)]
pub enum CoreBootstrapProductionError {
    Hir(CoreBootstrapHirStageError),
    Mir(CurrentConeMirStageError),
    Lir(CurrentConeLirStageError),
    StrongProfile(CoreBootstrapStrongProfileError),
    Warnings(super::CurrentConeDiagnosticSetError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Artifact(crate::CrossConeStrongIrArtifactProductionError),
    Publication(crate::CrossConeArtifactProductionError),
}

impl fmt::Display for CoreBootstrapProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hir(source) => source.fmt(formatter),
            Self::Mir(source) => source.fmt(formatter),
            Self::Lir(source) => source.fmt(formatter),
            Self::StrongProfile(source) => source.fmt(formatter),
            Self::Warnings(source) => source.fmt(formatter),
            Self::Producer(source) => source.fmt(formatter),
            Self::Cone(source) => source.fmt(formatter),
            Self::Artifact(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hir(source) => Some(source),
            Self::Mir(source) => Some(source),
            Self::Lir(source) => Some(source),
            Self::StrongProfile(source) => Some(source),
            Self::Warnings(source) => Some(source),
            Self::Producer(source) => Some(source),
            Self::Cone(source) => Some(source),
            Self::Artifact(source) => Some(source),
            Self::Publication(source) => Some(source),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreBootstrapStrongProfileError {
    HirOdr(scoop_hir::OdrFreeHirFoundationError),
}

impl fmt::Display for CoreBootstrapStrongProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOdr(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapStrongProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirOdr(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreBootstrapHirStageError {
    Sources(scoop_hir_lower::CurrentConeSourceError),
    Lowering(Vec<scoop_ast::Diagnostic>),
    Foundation(scoop_hir::HirFoundationBuildError),
    ProductionSection(scoop_hir::CoreBootstrapInterfaceBuildError),
    CoreClassifier(scoop_hir::CoreClosedExactLeafClassifierBuildError),
    SemanticWorld(scoop_hir::ImportedSemanticWorldBuildError),
    CrossConeSection(
        Box<
            scoop_hir::CrossConeHirInterfaceProductionError<
                scoop_hir::CrossConeHirProductionAuthorityError,
            >,
        >,
    ),
}

impl fmt::Display for CoreBootstrapHirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sources(source) => source.fmt(formatter),
            Self::Lowering(diagnostics) => write!(
                formatter,
                "trusted core HIR lowering failed with {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::CoreClassifier(source) => source.fmt(formatter),
            Self::CrossConeSection(source) => source.fmt(formatter),
            Self::SemanticWorld(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sources(source) => Some(source),
            Self::Lowering(_) => None,
            Self::Foundation(source) => Some(source),
            Self::ProductionSection(source) => Some(source),
            Self::CoreClassifier(source) => Some(source),
            Self::CrossConeSection(source) => Some(source.as_ref()),
            Self::SemanticWorld(source) => Some(source),
        }
    }
}
