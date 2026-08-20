//! Shared dependency projection and complete machine lowering outputs.

mod errors;
pub use errors::{CurrentConeLirStageError, CurrentConeMirStageError};

pub(super) struct CurrentConeMachineHir<'a> {
    pub output: &'a scoop_hir::DependencyHirOutput,
    pub production: &'a scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub public: &'a scoop_hir::CrossConeHirInterfaceSectionV1,
    pub classifier: &'a scoop_hir::NominalExactLeafClassifierV1,
}

pub(super) struct CurrentConeMirArtifacts {
    pub strong: scoop_mir::ConeMirInput,
    pub public: scoop_mir::CrossConeMirBridgeSectionV1,
}

impl CurrentConeMachineHir<'_> {
    pub fn lower_selected_mir(
        self,
        selected_callables: scoop_mir::SelectedExternalMirSet,
        optimization: scoop_mir_lower::MirOptimizationOptions,
    ) -> Result<CurrentConeMirArtifacts, CurrentConeMirStageError> {
        let mir =
            scoop_mir_lower::lower_current_cone(self.output, selected_callables, optimization)
                .map_err(CurrentConeMirStageError::Lowering)?;
        let foundation = mir.foundation();
        let production = scoop_mir_lower::lower_production_section(
            mir.module().cone,
            self.production,
            foundation,
        )
        .map_err(CurrentConeMirStageError::ProductionSection)?;
        let public = scoop_mir_lower::lower_cross_cone_bridge_section(
            mir.module().cone,
            self.public,
            self.classifier,
            foundation,
            mir.selected_callables(),
        )
        .map_err(CurrentConeMirStageError::CrossConeBridge)?;
        let shapes = self
            .output
            .output()
            .local
            .materialization()
            .roots()
            .iter()
            .map(|root| root.declaration().clone())
            .collect();
        let strong = scoop_mir::ConeMirInput::try_new(mir, production, shapes)
            .map_err(CurrentConeMirStageError::Sealing)?;
        Ok(CurrentConeMirArtifacts { strong, public })
    }
}

pub(super) fn lower_selected_lir(
    strong: &scoop_mir::ConeMirInput,
    public: &scoop_mir::CrossConeMirBridgeSectionV1,
    selected_callables: &scoop_lir::SelectedExternalLirSet,
    target: scoop_lir::LirTargetProfile,
    selected_layout: &scoop_lir::StrongProductionDependencySelectionV2<'_>,
    diagnostics: &impl scoop_identity::ExactTypeDiagnosticGraph,
) -> Result<
    (
        scoop_lir::ConeLirOutput,
        scoop_lir::CrossConeLirBridgeSectionV1,
    ),
    CurrentConeLirStageError,
> {
    let lir = scoop_lir_lower::lower_with_layout_dependencies(
        strong,
        selected_callables,
        target,
        selected_layout,
        diagnostics,
    )
    .map_err(CurrentConeLirStageError::Lowering)?;
    let public = scoop_lir_lower::lower_cross_cone_bridge_section(strong, public, &lir)
        .map_err(CurrentConeLirStageError::CrossConeBridge)?;
    Ok((lir, public))
}

#[cfg(test)]
mod tests;
