#[derive(Debug)]
pub enum LayoutProductionError {
    HirTypes(scoop_hir::CrossConeTypeSemanticsProductionError),
    MirExports(scoop_mir_lower::MirTypeBridgeExportProductionError),
    MirUses(scoop_mir_lower::MirTypeBridgeUseLoweringError),
    MirSection(scoop_mir::MirTypeBridgeSectionError),
    Shape(scoop_lir::ShapeLinkError),
    Selection(scoop_lir::LayoutAbiSectionError),
    Initialization(scoop_lir_lower::StrongProductionV2ProjectionError),
    Registration(Box<scoop_lir::ConeProductionWriterError>),
    LayoutExports(scoop_lir_lower::LayoutAbiExportLoweringError),
    LayoutUses(scoop_lir_lower::LayoutAbiDependencyLoweringError),
    LayoutJoin(scoop_lir::StrongProductionLayoutJoinError),
    Codegen(scoop_codegen::CodegenError),
    Artifact(crate::LayoutArtifactProductionError),
}

impl std::fmt::Display for LayoutProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source =
            std::error::Error::source(self).expect("every layout production error has a cause");
        write!(
            f,
            "cannot produce the current Cone layout artifact: {source}"
        )
    }
}

impl std::error::Error for LayoutProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::HirTypes(error) => error,
            Self::MirExports(error) => error,
            Self::MirUses(error) => error,
            Self::MirSection(error) => error,
            Self::Shape(error) => error,
            Self::Selection(error) => error,
            Self::Initialization(error) => error,
            Self::Registration(error) => error.as_ref(),
            Self::LayoutExports(error) => error,
            Self::LayoutUses(error) => error,
            Self::LayoutJoin(error) => error,
            Self::Codegen(error) => error,
            Self::Artifact(error) => error,
        })
    }
}
