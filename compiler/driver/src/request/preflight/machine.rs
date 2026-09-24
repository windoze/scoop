//! Shared dependency projection, machine lowering, and strong sealing.

mod errors;
pub use errors::{CurrentConeLirStageError, CurrentConeMirStageError};

pub(super) struct CurrentConeMachineHir<'a> {
    pub output: &'a scoop_hir::DependencyHirOutput,
    pub production: &'a scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub public: &'a scoop_hir::CrossConeHirInterfaceSectionV1,
    pub classifier: &'a scoop_hir::NominalExactLeafClassifierV1,
}

pub(super) struct CurrentConeMirArtifacts {
    pub strong: scoop_mir::SingleConeStrongMirInput,
    pub selected_callables: scoop_mir::SelectedExternalMirSet,
    pub public: scoop_mir::CrossConeMirBridgeSectionV1,
}

impl CurrentConeMachineHir<'_> {
    pub fn lower_selected_mir(
        self,
        selected_callables: scoop_mir::SelectedExternalMirSet,
    ) -> Result<CurrentConeMirArtifacts, CurrentConeMirStageError> {
        let mir = scoop_mir_lower::lower_current_cone(self.output, selected_callables)
            .map_err(CurrentConeMirStageError::Lowering)?;
        let (module, selected_callables) = mir.into_parts();
        let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&module)
            .map_err(CurrentConeMirStageError::Foundation)?;
        let production =
            scoop_mir_lower::lower_production_section(module.cone, self.production, &foundation)
                .map_err(CurrentConeMirStageError::ProductionSection)?;
        let public = scoop_mir_lower::lower_cross_cone_bridge_section(
            module.cone,
            self.public,
            self.classifier,
            &foundation,
            &selected_callables,
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
        let strong = scoop_mir::SingleConeStrongMirInput::try_new(
            module,
            foundation,
            production,
            shapes,
            scoop_mir::StrongExternalCallableInput::Selected(&selected_callables),
        )
        .map_err(CurrentConeMirStageError::Sealing)?;
        Ok(CurrentConeMirArtifacts {
            strong,
            selected_callables,
            public,
        })
    }
}

pub(super) fn lower_selected_lir(
    strong: &scoop_mir::SingleConeStrongMirInput,
    public: &scoop_mir::CrossConeMirBridgeSectionV1,
    runtime_string: scoop_lir_lower::RuntimeStringDescriptor,
    selected_callables: &scoop_lir::SelectedExternalLirSet,
    target: scoop_lir::LirTargetProfile,
) -> Result<
    (
        scoop_lir::SingleConeStrongLirOutput,
        scoop_lir::CrossConeLirBridgeSectionV1,
    ),
    CurrentConeLirStageError,
> {
    let lir = scoop_lir_lower::lower(strong, runtime_string, selected_callables, target)
        .map_err(CurrentConeLirStageError::Lowering)?;
    let public = scoop_lir_lower::lower_cross_cone_bridge_section(strong, public, &lir)
        .map_err(CurrentConeLirStageError::CrossConeBridge)?;
    Ok((lir, public))
}

#[cfg(test)]
mod tests;
