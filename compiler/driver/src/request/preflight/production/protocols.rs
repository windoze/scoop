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
        cone: scoop_slib::ConeRecord,
        temporary_parent: &Path,
        dump: &mut Option<EmittedStageDump>,
    ) -> Result<scoop_slib::AssembledCrossConeLayoutStrongArtifactV1, CurrentConeProductionFailure>
    {
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
        layout::assemble(
            hir,
            request,
            cone,
            temporary_parent,
            selected,
            runtime_string,
            dump,
        )
    }
}

pub(super) fn type_identities(
    hir: &current_hir::CurrentConeHirArtifacts,
    mir: &machine::CurrentConeMirArtifacts,
    dependencies: &scoop_slib::ValidatedCrossConeSemanticClosure,
    coordinate: &ConeCoordinate,
) -> Result<
    (scoop_identity::ValidatedIdentityGraph, Vec<ConeCoordinate>),
    scoop_identity::IdentityValidationError,
> {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending.register_authority(hir.hir.output().export.cone)?;
    hir.foundation.register_identities(&mut pending)?;
    mir.strong
        .foundation()
        .as_canonical()
        .register_identities(&mut pending)?;
    let mut coordinates = vec![coordinate.clone()];
    for (coordinate, identities) in dependencies.identity_inputs() {
        pending.register_external_graph_authorities(identities)?;
        coordinates.push(coordinate.clone());
    }
    Ok((pending.finish()?, coordinates))
}
