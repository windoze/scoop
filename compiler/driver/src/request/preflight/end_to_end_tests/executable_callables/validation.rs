use super::*;

pub(super) fn check_machine_input(request: SingleConeBuildRequest, selected_count: usize) {
    let loaded = request.load_preflight().unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let ValidatedCompilerProtocols::Imported(inputs) = request.protocols() else {
        panic!("ordinary source imports protocol declarations")
    };
    let closure = request.dependencies().semantic();
    let world = closure.imported_semantic_world().unwrap();
    let hir = current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        parsed.sources(),
        inputs.as_ref().clone().into(),
        &world,
        Default::default(),
    )
    .unwrap();
    let machine_dependencies = request
        .dependencies()
        .read_machine_dependencies(hir.machine_dependency_providers())
        .unwrap();
    let closure = machine_dependencies.semantic();
    let projected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    // Core helpers also support materialized generic bodies such as the
    // Throwable snapshot array. Count calls to the fixture providers here.
    assert_eq!(
        projected
            .callables()
            .iter()
            .filter(|callable| callable.provider() != ConeIdentity::CORE)
            .count(),
        selected_count
    );
    scoop_mir_lower::lower_current_cone(&hir.hir, projected, Default::default(), None).unwrap();
    if selected_count != 0 {
        let consumer = hir.hir.output().local.module().cone;
        let missing = scoop_mir::SelectedExternalMirSet::empty(consumer);
        assert!(matches!(
            scoop_mir_lower::lower_current_cone(&hir.hir, missing, Default::default(), None),
            Err(scoop_mir_lower::CurrentConeMirLoweringError::MissingDependencyMirCallable { .. })
        ));
    }
}
