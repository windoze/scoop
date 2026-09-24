use super::*;

impl ValidatedCompilerProtocols {
    pub(super) fn hir_input(&self) -> scoop_hir_lower::CoreProtocolInput {
        match self {
            Self::CurrentDeclarations => scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
            Self::Imported(inputs) => inputs.as_ref().clone().into(),
        }
    }

    pub(super) fn lower_machine(
        &self,
        hir: current_hir::CurrentConeHirArtifacts,
        request: &ValidatedSingleConeBuildRequest<'_>,
        dump: &mut Option<EmittedStageDump>,
    ) -> Result<CrossConeStrongIrProductionV1, CurrentConeProductionFailure> {
        let selected = request
            .dependencies()
            .semantic()
            .project_dependency_callables_to_mir(&hir.hir)
            .map_err(CurrentConeMirStageError::DependencyProjection)
            .map_err(CurrentConeProductionFailure::Mir)?;
        let (selected, runtime_string) = match self {
            Self::CurrentDeclarations => {
                (selected, scoop_lir_lower::RuntimeStringDescriptor::Local)
            }
            Self::Imported(inputs) => {
                let needs_cycle = !hir
                    .hir
                    .output()
                    .local
                    .module()
                    .initialization_units
                    .is_empty();
                let protocols = inputs.protocols();
                let closure = request.dependencies().semantic();
                let selected = if needs_cycle {
                    closure
                        .select_initialization_cycle(
                            selected,
                            protocols.exceptions().initialization_cycle_thrower(),
                        )
                        .map_err(CurrentConeMirStageError::Initialization)
                        .map_err(CurrentConeProductionFailure::Mir)?
                } else {
                    selected
                };
                let string = protocols.fundamental_types().string();
                let descriptor = request
                    .dependencies()
                    .semantic()
                    .project_source_type_descriptor(string.provider(), string.persistent())
                    .map_err(CurrentConeLirStageError::TypeDescriptor)
                    .map_err(CurrentConeProductionFailure::Lir)?;
                (
                    selected,
                    scoop_lir_lower::RuntimeStringDescriptor::External(descriptor),
                )
            }
        };
        lower_machine(hir, request, selected, runtime_string, dump)
    }
}

fn lower_machine(
    hir: current_hir::CurrentConeHirArtifacts,
    request: &ValidatedSingleConeBuildRequest<'_>,
    selected: scoop_mir::SelectedExternalMirSet,
    runtime_string: scoop_lir_lower::RuntimeStringDescriptor,
    dump: &mut Option<EmittedStageDump>,
) -> Result<CrossConeStrongIrProductionV1, CurrentConeProductionFailure> {
    let closure = request.dependencies().semantic();
    let mir = hir
        .machine_input()
        .lower_selected_mir(selected)
        .map_err(CurrentConeProductionFailure::Mir)?;
    if dump.is_none() {
        *dump = capture_stage_dump(request.emit(), StageDumpKind::Mir, || {
            scoop_mir::dump(mir.strong.module())
        });
    }
    let selected = closure
        .project_dependency_callables_to_lir(&mir.selected_callables)
        .map_err(CurrentConeLirStageError::DependencyProjection)
        .map_err(CurrentConeProductionFailure::Lir)?;
    let (lir, lir_public) = machine::lower_selected_lir(
        &mir.strong,
        &mir.public,
        runtime_string,
        &selected,
        request.target().lir_target(),
    )
    .map_err(CurrentConeProductionFailure::Lir)?;
    if dump.is_none() {
        *dump = capture_stage_dump(request.emit(), StageDumpKind::Lir, || {
            scoop_lir::dump(lir.module())
        });
    }
    hir.seal_strong_profile(mir.strong, mir.public, lir, lir_public)
        .map_err(CurrentConeProductionFailure::StrongProfile)
}
