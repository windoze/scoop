use super::*;

#[derive(Debug)]
pub enum CoreBootstrapProductionError {
    Hir(CoreBootstrapHirStageError),
    Mir(CoreBootstrapMirStageError),
    Lir(CoreBootstrapLirStageError),
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
pub enum CoreBootstrapLirStageError {
    Lowering(scoop_lir_lower::StrongLirLoweringError),
    CrossConeBridge(scoop_lir_lower::CrossConeLirBridgeLoweringError),
}

impl fmt::Display for CoreBootstrapLirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(source) => source.fmt(formatter),
            Self::CrossConeBridge(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapLirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(source) => Some(source),
            Self::CrossConeBridge(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreBootstrapMirStageError {
    MissingCoreShapeSupportPlan,
    Lowering(scoop_mir_lower::DefinedCoreMirLoweringError),
    Foundation(scoop_mir::OdrFreeMirFoundationProjectionError),
    ProductionSection(scoop_mir_lower::MirProductionLoweringError),
    CrossConeBridge(scoop_mir_lower::CrossConeMirBridgeLoweringError),
    Sealing(scoop_mir::SingleConeStrongMirInputError),
}

impl fmt::Display for CoreBootstrapMirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoreShapeSupportPlan => {
                formatter.write_str("trusted core HIR output has no core shape-support plan")
            }
            Self::Lowering(source) => source.fmt(formatter),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::CrossConeBridge(source) => source.fmt(formatter),
            Self::Sealing(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapMirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::MissingCoreShapeSupportPlan => return None,
            Self::Lowering(source) => source,
            Self::Foundation(source) => source,
            Self::ProductionSection(source) => source,
            Self::CrossConeBridge(source) => source,
            Self::Sealing(source) => source,
        })
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
