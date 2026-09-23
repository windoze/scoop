use super::*;

pub(super) fn with_production(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_bytes: &[u8],
    source: &str,
    run: impl FnOnce(
        scoop_lir_lower::LayoutAbiExportInputV1<'_>,
        scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    ),
) {
    let root = sysroot.join("layout-library");
    write_manifest_cone(&root, "dev.example", "layout-library", "library", source);
    let loaded = build_manifest_request(
        sysroot,
        target,
        &root,
        &sysroot.join("output/layout-probe.slib"),
        vec![],
        vec![],
    )
    .load_preflight(DecodeLimits::default())
    .unwrap();
    let request = loaded.validate().unwrap();
    let parsed = request.parse_current_sources().unwrap();
    let ValidatedCompilerProtocols::Imported(inputs) = request.protocols() else {
        panic!("ordinary source imports its actual language declarations")
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
    let source = scoop_hir_lower::produce_cross_cone_type_semantics(
        &hir.hir,
        &hir.cross_cone_section,
        &mut meter(),
    )
    .unwrap();
    let selected = closure
        .project_dependency_callables_to_mir(hir.hir.imported_dependencies())
        .unwrap();
    let selected = if hir
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
    };
    let mir = hir.machine_input().lower_selected_mir(selected).unwrap();
    let selected = closure
        .project_dependency_callables_to_lir(&mir.selected_callables)
        .unwrap();
    let source_string = inputs.protocols().fundamental_types().string();
    let string = closure
        .project_source_type_descriptor(source_string.provider(), source_string.persistent())
        .unwrap();
    let front = DecodedSlibEnvelope::open(
        core_bytes,
        DecodeLimits::default(),
        target.lir_target_selection(),
    )
    .unwrap()
    .validate_graph()
    .unwrap()
    .decode_cross_cone_hir_front_sections()
    .unwrap();
    let coordinate = ConeCoordinate::new("dev.example", "layout-library", "0.1.0").unwrap();
    let coordinates = [front.coordinate().clone(), coordinate.clone()];
    let (source_graph, _) = identities(&hir.hir, &mir.strong, None, &front);
    let diagnostics = scoop_identity::ExactTypeDiagnosticCatalog::try_new(
        &source_graph,
        &coordinates,
        &mut meter(),
    )
    .unwrap();
    let lir = scoop_lir_lower::lower_with_diagnostics(
        &mir.strong,
        scoop_lir_lower::RuntimeStringDescriptor::External(string),
        &selected,
        target.lir_target(),
        &diagnostics,
        &mut meter(),
    )
    .unwrap();
    let (graph, core_lir) = identities(&hir.hir, &mir.strong, Some(&lir), &front);
    let types = dependencies::mir_types(&mir.strong, &graph);
    let input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
        hir: &hir.hir,
        public: &hir.cross_cone_section,
        source: &source,
        mir: &mir.strong,
        ordinary: &mir.public,
        identities: &graph,
    };
    let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
        types: &[&types],
        callables: &[],
        dispatch: &[],
    };
    let bridge = scoop_mir_lower::lower_type_bridge_exports(
        input,
        dependencies,
        mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
        &mut meter(),
    )
    .unwrap();
    let projected = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
        input,
        dependencies,
        mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
        &mut meter(),
    )
    .unwrap();
    bridge
        .validate_sources(mir.strong.module().cone, &graph, &projected, &mut meter())
        .unwrap();
    let uses =
        mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(&projected).unwrap();
    assert!(!uses.is_empty());
    assert!(
        uses.iter()
            .all(|usage| usage.provider() == ConeIdentity::CORE
                && matches!(usage.target(), mir::MirTypeBridgeTargetV1::Type(_)))
    );
    assert_eq!(
        mir::MirTypeBridgeSectionSourceAuthorityV1::local_initialization_units(&projected)
            .unwrap()
            .len(),
        mir.strong.materialization().initialization_roots().len(),
    );
    let layouts = dependencies::layouts(&types, &core_lir, &graph, target.lir_target());
    let selected = lir::StrongProductionDependencySelectionV2::empty(
        lir.module().cone,
        target.lir_target(),
        &mut meter(),
    )
    .unwrap();
    let registration = lir
        .build_production_section_v2(
            coordinate.clone(),
            lir::EntryProductionSourceV1::Library,
            &selected,
            &[],
            &mut meter(),
        )
        .unwrap();
    run(
        scoop_lir_lower::LayoutAbiExportInputV1 {
            mir: &mir.strong,
            lir: &lir,
            bridge: &bridge,
            registration: &registration,
            identities: &graph,
            coordinates: &coordinates,
        },
        scoop_lir_lower::LayoutAbiExportDependenciesV1 {
            layouts: &[&layouts],
            callables: &[],
        },
    );
}

fn identities(
    hir: &hir::DependencyHirOutput,
    mir: &mir::SingleConeStrongMirInput,
    lir: Option<&lir::SingleConeStrongLirOutput>,
    core: &scoop_slib::DecodedCrossConeHirFrontSections<'_>,
) -> (ValidatedIdentityGraph, lir::OdrFreeLirFoundation) {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    core.hir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.mir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.lir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.hir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    core.mir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    core.lir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    let mut core_graph = pending.finish().unwrap();
    let core_lir: lir::DecodedLirFoundation = decoded(core.lir_foundation_wire());
    let core_lir = lir::OdrFreeLirFoundation::from_validated(
        core_lir
            .validate(ConeIdentity::CORE, &mut core_graph, &mut meter())
            .unwrap(),
    )
    .unwrap();
    let hir: hir::DecodedHirFoundation =
        decoded(&hir::CanonicalHirFoundation::from_type_semantics_output(hir).unwrap());
    let provider = mir.module().cone;
    let mir: mir::DecodedMirFoundation = decoded(mir.foundation().as_canonical());
    let lir_foundation: Option<lir::DecodedLirFoundation> =
        lir.map(|lir| decoded(lir.foundation().as_canonical()));
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    if let Some(foundation) = &lir_foundation {
        foundation.register_identities(&mut pending).unwrap();
    }
    pending
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    hir.resolve_identities(&mut pending).unwrap();
    mir.resolve_identities(&mut pending).unwrap();
    if let Some(foundation) = &lir_foundation {
        foundation.resolve_identities(&mut pending).unwrap();
    }
    (pending.finish().unwrap(), core_lir)
}
