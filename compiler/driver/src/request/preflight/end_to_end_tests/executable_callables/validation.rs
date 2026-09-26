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
    )
    .unwrap();
    let projected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    assert_eq!(projected.len(), selected_count);
    let with_initialization = |selected| {
        if hir
            .hir
            .output()
            .local
            .module()
            .initialization_units
            .is_empty()
        {
            selected
        } else {
            closure
                .select_initialization_cycle(
                    selected,
                    inputs
                        .protocols()
                        .exceptions()
                        .initialization_cycle_thrower(),
                )
                .unwrap()
        }
    };
    scoop_mir_lower::lower_current_cone(&hir.hir, with_initialization(projected)).unwrap();
    if selected_count != 0 {
        let consumer = hir.hir.output().local.module().cone;
        let missing = scoop_mir::SelectedExternalMirSet::empty(consumer);
        assert!(matches!(
            scoop_mir_lower::lower_current_cone(&hir.hir, with_initialization(missing)),
            Err(scoop_mir_lower::CurrentConeMirLoweringError::MissingDependencyMirCallable { .. })
        ));
    }
}
