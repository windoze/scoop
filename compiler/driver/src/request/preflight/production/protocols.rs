use super::*;
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub(super) enum CurrentProtocols<'stage, 'artifact> {
    Declared,
    Imported(&'stage ValidatedTrustedCoreArtifact<'artifact>),
}

impl<'stage> CurrentProtocols<'stage, '_> {
    pub fn hir_input(self) -> Result<scoop_hir_lower::CoreProtocolInput, CurrentConeHirStageError> {
        match self {
            Self::Declared => Ok(scoop_hir_lower::CoreProtocolInput::CurrentDeclarations),
            Self::Imported(core) => core
                .import_core_inputs()
                .map(Into::into)
                .map_err(CurrentConeHirStageError::CoreInterface),
        }
    }

    pub fn defined_symbols(self) -> Cow<'stage, scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1> {
        match self {
            Self::Declared => {
                Cow::Owned(scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap())
            }
            Self::Imported(core) => Cow::Borrowed(core.defined_symbols()),
        }
    }

    pub fn lower_machine(
        self,
        hir: current_hir::CurrentConeHirArtifacts,
        request: &ValidatedCoreOnlyBuildRequest<'_>,
        dump: &mut Option<EmittedStageDump>,
    ) -> Result<CrossConeStrongIrProductionV1, CurrentConeProductionFailure> {
        match self {
            Self::Declared => lower_machine(
                hir,
                request,
                scoop_mir::CurrentMirProtocolDeclarations,
                |_| Ok(LirProtocols::Declared),
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
                    .project_initialization_protocol_to_mir(needs_cycle)
                    .map_err(CurrentConeMirStageError::Projection)
                    .map_err(CurrentConeProductionFailure::Mir)?;
                lower_machine(
                    hir,
                    request,
                    selected,
                    |mir| {
                        core.project_core_callables_to_lir(mir)
                            .map(LirProtocols::Imported)
                            .map_err(CurrentConeLirStageError::Projection)
                    },
                    dump,
                )
            }
        }
    }
}

enum LirProtocols<'artifact> {
    Declared,
    Imported(scoop_lir::SelectedImportedLirSet<'artifact>),
}

impl LirProtocols<'_> {
    fn as_input(&self) -> scoop_lir_lower::StrongImportedCoreLirInput<'_> {
        match self {
            Self::Declared => scoop_lir_lower::StrongImportedCoreLirInput::Unused,
            Self::Imported(selected) => {
                scoop_lir_lower::StrongImportedCoreLirInput::Selected(selected)
            }
        }
    }
}

fn lower_machine<'protocol, P: scoop_mir::MirProtocolSelection>(
    hir: current_hir::CurrentConeHirArtifacts,
    request: &ValidatedCoreOnlyBuildRequest<'_>,
    protocols: P,
    project_lir: impl FnOnce(&P) -> Result<LirProtocols<'protocol>, CurrentConeLirStageError>,
    dump: &mut Option<EmittedStageDump>,
) -> Result<CrossConeStrongIrProductionV1, CurrentConeProductionFailure> {
    let closure = request.dependencies().semantic();
    let mir = hir
        .machine_input()
        .lower_mir(protocols, closure)
        .map_err(CurrentConeProductionFailure::Mir)?;
    if dump.is_none() {
        *dump = capture_stage_dump(request.emit(), StageDumpKind::Mir, || {
            scoop_mir::dump(mir.strong.module())
        });
    }
    let protocols = project_lir(&mir.protocols).map_err(CurrentConeProductionFailure::Lir)?;
    let (lir, lir_public) = machine::lower_lir(
        &mir.strong,
        &mir.public,
        protocols.as_input(),
        &mir.dependencies,
        closure,
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
