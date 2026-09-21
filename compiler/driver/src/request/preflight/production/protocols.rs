use super::*;
use std::borrow::Cow;

impl ValidatedCompilerProtocols<'_> {
    pub(super) fn hir_input(
        &self,
    ) -> Result<scoop_hir_lower::CoreProtocolInput, CurrentConeHirStageError> {
        match self {
            Self::CurrentDeclarations => {
                Ok(scoop_hir_lower::CoreProtocolInput::CurrentDeclarations)
            }
            Self::Imported(core) => core
                .import_core_inputs()
                .map(Into::into)
                .map_err(CurrentConeHirStageError::CoreInterface),
        }
    }

    pub(super) fn defined_symbols(
        &self,
    ) -> Cow<'_, scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1> {
        match self {
            Self::CurrentDeclarations => {
                Cow::Owned(scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap())
            }
            Self::Imported(core) => Cow::Borrowed(core.defined_symbols()),
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
            .project_dependency_callables_to_mir(hir.hir.imported_dependencies())
            .map_err(CurrentConeMirStageError::DependencyProjection)
            .map_err(CurrentConeProductionFailure::Mir)?;
        match self {
            Self::CurrentDeclarations => lower_machine(
                hir,
                request,
                selected,
                |_, selected| Ok((scoop_lir_lower::RuntimeStringDescriptor::Local, selected)),
                dump,
            ),
            Self::Imported(core) => {
                let needs_cycle = !hir
                    .hir
                    .output()
                    .local
                    .module()
                    .initialization_units
                    .is_empty();
                let selected = core
                    .project_initialization_protocol_to_mir(selected, needs_cycle)
                    .map_err(CurrentConeMirStageError::Projection)
                    .map_err(CurrentConeProductionFailure::Mir)?;
                lower_machine(
                    hir,
                    request,
                    selected,
                    |mir, selected| {
                        core.project_core_callables_to_lir(mir, selected)
                            .map(|(descriptor, selected)| {
                                (
                                    scoop_lir_lower::RuntimeStringDescriptor::External(descriptor),
                                    selected,
                                )
                            })
                            .map_err(CurrentConeLirStageError::Projection)
                    },
                    dump,
                )
            }
        }
    }
}

fn lower_machine(
    hir: current_hir::CurrentConeHirArtifacts,
    request: &ValidatedSingleConeBuildRequest<'_>,
    selected: scoop_mir::SelectedExternalMirSet,
    project_lir: impl FnOnce(
        &scoop_mir::SelectedExternalMirSet,
        scoop_lir::SelectedExternalLirSet,
    ) -> Result<
        (
            scoop_lir_lower::RuntimeStringDescriptor,
            scoop_lir::SelectedExternalLirSet,
        ),
        CurrentConeLirStageError,
    >,
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
    let (runtime_string, selected) = project_lir(&mir.selected_callables, selected)
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
