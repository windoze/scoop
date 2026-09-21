use std::fmt;

#[derive(Debug)]
pub enum OrdinaryConeProductionError {
    Hir(super::OrdinaryConeHirStageError),
    Mir(super::CurrentConeMirStageError),
    Lir(super::CurrentConeLirStageError),
    StrongProfile(super::OrdinaryConeStrongProfileError),
    Warnings(crate::request::CurrentConeDiagnosticSetError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Artifact(crate::CrossConeStrongIrArtifactProductionError),
    Publication(crate::CrossConeArtifactProductionError),
}

impl fmt::Display for OrdinaryConeProductionError {
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

impl std::error::Error for OrdinaryConeProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Hir(source) => source,
            Self::Mir(source) => source,
            Self::Lir(source) => source,
            Self::StrongProfile(source) => source,
            Self::Warnings(source) => source,
            Self::Producer(source) => source,
            Self::Cone(source) => source,
            Self::Artifact(source) => source,
            Self::Publication(source) => source,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinaryConeStrongProfileError {
    HirOdr(scoop_hir::OdrFreeHirFoundationError),
}

impl fmt::Display for OrdinaryConeStrongProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOdr(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeStrongProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirOdr(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum OrdinaryConeHirStageError {
    SemanticWorld(scoop_hir::ImportedSemanticWorldBuildError),
    CoreInterface(scoop_hir::CoreInterfaceImportError),
    CoreClassifier(scoop_hir::CoreClosedExactLeafClassifierBuildError),
    Input(scoop_hir_lower::CurrentConeSourceError),
    Lowering(Vec<scoop_ast::Diagnostic>),
    Foundation(scoop_hir::HirFoundationBuildError),
    ProductionSection(scoop_hir::CoreBootstrapInterfaceBuildError),
    CrossConeSection(
        scoop_hir::CrossConeHirInterfaceProductionError<
            scoop_hir::CrossConeHirProductionAuthorityError,
        >,
    ),
}

impl fmt::Display for OrdinaryConeHirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticWorld(source) => source.fmt(formatter),
            Self::CoreInterface(source) => source.fmt(formatter),
            Self::CoreClassifier(source) => source.fmt(formatter),
            Self::Input(source) => source.fmt(formatter),
            Self::Lowering(diagnostics) => write!(
                formatter,
                "ordinary HIR lowering failed with {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::CrossConeSection(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticWorld(source) => Some(source),
            Self::CoreInterface(source) => Some(source),
            Self::CoreClassifier(source) => Some(source),
            Self::Input(source) => Some(source),
            Self::Lowering(_) => None,
            Self::Foundation(source) => Some(source),
            Self::ProductionSection(source) => Some(source),
            Self::CrossConeSection(source) => Some(source),
        }
    }
}
