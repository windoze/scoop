use super::*;

pub(super) fn lower(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    case: &str,
) -> (ConeCoordinate, scoop_lir::ConeLirOutput) {
    let name = format!("release-machine-{}", case.replace('/', "-"));
    let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
    let root = sysroot.join(&name);
    let source = std::fs::read_to_string(
        crate::workspace_root()
            .join("tests/fixtures/m24-release-blocks")
            .join(case)
            .join("program.scoop"),
    )
    .unwrap();
    write_manifest_cone(&root, "dev.example", &name, "library", &source);
    let loaded = build_manifest_request(
        sysroot,
        target,
        &root,
        &sysroot.join(format!("{name}.slib")),
        Vec::new(),
        Vec::new(),
    )
    .load_preflight()
    .unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let world = request
        .dependencies()
        .semantic()
        .imported_semantic_world()
        .unwrap();
    let hir = parsed
        .lower_hir(
            scoop_identity::RequestedConeKind::Library,
            request.protocols(),
            &world,
        )
        .unwrap();
    let closure = request.dependencies().semantic();
    let selected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    let mir = hir
        .machine_input()
        .lower_selected_mir(selected, Default::default())
        .unwrap();
    let selected = closure
        .project_dependency_callables_to_lir(mir.strong.selected_callables())
        .unwrap();
    let dependencies = closure.layout_dependencies().collect::<Vec<_>>();
    let layouts =
        production::select_lir_dependencies(&mir.strong, [], &dependencies, target.lir_target())
            .unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(coordinate.identity().unwrap())
        .unwrap();
    hir.foundation.register_identities(&mut pending).unwrap();
    mir.strong
        .foundation()
        .register_identities(&mut pending)
        .unwrap();
    let mut coordinates = vec![coordinate.clone()];
    for (coordinate, identities) in closure.identity_inputs() {
        pending
            .register_external_graph_authorities(identities)
            .unwrap();
        coordinates.push(coordinate.clone());
    }
    let identities = pending.finish().unwrap();
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&identities, &coordinates).unwrap();
    let (lir, _) = machine::lower_selected_lir(
        &mir.strong,
        &mir.public,
        &selected,
        target.lir_target(),
        &layouts,
        &diagnostics,
    )
    .unwrap();
    (coordinate, lir)
}
