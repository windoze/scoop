//! Shared dependency projection, machine lowering, and strong sealing.

mod errors;
pub use errors::{CurrentConeLirStageError, CurrentConeMirStageError};

pub(super) struct CurrentConeMachineHir<'a> {
    pub output: &'a scoop_hir::DependencyHirOutput,
    pub production: &'a scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub public: &'a scoop_hir::CrossConeHirInterfaceSectionV1,
    pub classifier: &'a scoop_hir::CoreClosedExactLeafClassifierV1,
}

pub(super) struct CurrentConeMirArtifacts<P: scoop_mir::MirProtocolSelection> {
    pub strong: scoop_mir::SingleConeStrongMirInput,
    pub protocols: P,
    pub dependencies: scoop_mir::SelectedDependencyMirSet,
    pub public: scoop_mir::CrossConeMirBridgeSectionV1,
}

impl CurrentConeMachineHir<'_> {
    pub fn lower_mir<P: scoop_mir::MirProtocolSelection>(
        self,
        protocols: P,
        closure: &scoop_slib::ValidatedCrossConeSemanticClosure<'_>,
    ) -> Result<CurrentConeMirArtifacts<P>, CurrentConeMirStageError> {
        let selected = closure
            .project_dependency_callables_to_mir(self.output.imported_dependencies())
            .map_err(CurrentConeMirStageError::DependencyProjection)?;
        self.lower_selected_mir(protocols, selected)
    }

    pub fn lower_selected_mir<P: scoop_mir::MirProtocolSelection>(
        self,
        protocols: P,
        dependencies: scoop_mir::SelectedDependencyMirSet,
    ) -> Result<CurrentConeMirArtifacts<P>, CurrentConeMirStageError> {
        let mir = scoop_mir_lower::lower_current_cone(self.output, protocols, dependencies)
            .map_err(CurrentConeMirStageError::Lowering)?;
        let (module, protocols, dependencies) = mir.into_parts();
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
            &dependencies,
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
        let strong = scoop_mir::SingleConeStrongMirInput::try_new_with_dependencies(
            module,
            foundation,
            production,
            shapes,
            protocols.as_strong_input(),
            scoop_mir::StrongImportedDependencyInput::Selected(&dependencies),
        )
        .map_err(CurrentConeMirStageError::Sealing)?;
        Ok(CurrentConeMirArtifacts {
            strong,
            protocols,
            dependencies,
            public,
        })
    }
}

pub(super) fn lower_lir(
    strong: &scoop_mir::SingleConeStrongMirInput,
    public: &scoop_mir::CrossConeMirBridgeSectionV1,
    protocols: scoop_lir_lower::StrongImportedCoreLirInput<'_>,
    dependencies: &scoop_mir::SelectedDependencyMirSet,
    closure: &scoop_slib::ValidatedCrossConeSemanticClosure<'_>,
    target: scoop_lir::LirTargetProfile,
) -> Result<
    (
        scoop_lir::SingleConeStrongLirOutput,
        scoop_lir::CrossConeLirBridgeSectionV1,
    ),
    CurrentConeLirStageError,
> {
    let selected = closure
        .project_dependency_callables_to_lir(dependencies)
        .map_err(CurrentConeLirStageError::DependencyProjection)?;
    lower_selected_lir(strong, public, protocols, &selected, target)
}

pub(super) fn lower_selected_lir(
    strong: &scoop_mir::SingleConeStrongMirInput,
    public: &scoop_mir::CrossConeMirBridgeSectionV1,
    protocols: scoop_lir_lower::StrongImportedCoreLirInput<'_>,
    dependencies: &scoop_lir::SelectedDependencyLirSet,
    target: scoop_lir::LirTargetProfile,
) -> Result<
    (
        scoop_lir::SingleConeStrongLirOutput,
        scoop_lir::CrossConeLirBridgeSectionV1,
    ),
    CurrentConeLirStageError,
> {
    let lir = scoop_lir_lower::lower_with_dependencies(
        strong,
        protocols,
        scoop_lir_lower::StrongImportedDependencyLirInput::Selected(dependencies),
        target,
    )
    .map_err(CurrentConeLirStageError::Lowering)?;
    let public = scoop_lir_lower::lower_cross_cone_bridge_section(strong, public, &lir)
        .map_err(CurrentConeLirStageError::CrossConeBridge)?;
    Ok((lir, public))
}

#[cfg(test)]
mod tests;
