use super::*;

impl ValidatedCompilerProtocols {
    pub(super) fn hir_input(&self) -> scoop_hir_lower::CoreProtocolInput {
        match self {
            Self::CurrentDeclarations => scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
            Self::Imported(inputs) => inputs.as_ref().clone().into(),
        }
    }
}

pub(super) fn lower_machine(
    hir: current_hir::CurrentConeHirArtifacts,
    request: &ValidatedSingleConeBuildRequest<'_>,
    cone: scoop_slib::ConeRecord,
    temporary_parent: &Path,
    dump: &mut Vec<EmittedStageDump>,
    stop_after: Option<StageDumpKind>,
) -> Result<
    ControlFlow<(), scoop_slib::AssembledCrossConeLayoutArtifactV1>,
    CurrentConeProductionFailure,
> {
    let dependencies = request
        .dependencies()
        .read_machine_dependencies(hir.machine_dependency_providers())
        .map_err(CurrentConeProductionFailure::Dependencies)?;
    layout::assemble(
        hir,
        request,
        cone,
        temporary_parent,
        dependencies,
        dump,
        stop_after,
    )
}

pub(super) fn type_identities(
    hir: &current_hir::CurrentConeHirArtifacts,
    mir: &machine::CurrentConeMirArtifacts,
    dependencies: &scoop_slib::ValidatedCrossConeSemanticClosure,
    declarations: &scoop_slib::CompileDependencyClosure<'_>,
    coordinate: &ConeCoordinate,
) -> Result<
    (scoop_identity::ValidatedIdentityGraph, Vec<ConeCoordinate>),
    scoop_identity::IdentityValidationError,
> {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending.register_authority(hir.hir.output().export.cone)?;
    hir.foundation.register_identities(&mut pending)?;
    mir.strong.foundation().register_identities(&mut pending)?;
    let mut coordinates = vec![coordinate.clone()];
    for (coordinate, identities) in declarations.identity_inputs() {
        pending.register_external_graph_authorities(identities)?;
        coordinates.push(coordinate.clone());
    }
    for (_, identities) in dependencies.identity_inputs() {
        pending.register_external_graph_authorities(identities)?;
    }
    Ok((pending.finish()?, coordinates))
}
