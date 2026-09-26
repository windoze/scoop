use super::*;

pub(super) mod identity;
pub(super) mod physical;
use identity::identities;

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
    with_inspection(sysroot, target, core_bytes, source, |_, _| {}, run);
}

pub(super) fn with_inspection(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_bytes: &[u8],
    source: &str,
    inspect: impl FnOnce(
        scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
        &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
    ),
    run: impl FnOnce(
        scoop_lir_lower::LayoutAbiExportInputV1<'_>,
        scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    ),
) {
    with_pair(
        sysroot,
        target,
        core_bytes,
        source,
        None,
        None,
        |mir, projection, lir, dependencies, _| {
            inspect(mir, projection);
            run(lir, dependencies);
        },
    );
}

pub(super) fn with_pair(
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    core_bytes: &[u8],
    source: &str,
    provider_exports: Option<(
        &mir::MirTypeBridgeExportConstituentsV1,
        &lir::LayoutAbiExportConstituentsV1,
    )>,
    layout_provider: Option<&lir::ShapeLinkProviderV1<'_>>,
    run: impl FnOnce(
        scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
        &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
        scoop_lir_lower::LayoutAbiExportInputV1<'_>,
        scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
        &[scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1],
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
    .load_preflight()
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
    let selected = closure
        .project_dependency_callables_to_mir(&hir.hir)
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
    let front = DecodedSlibEnvelope::open(core_bytes, target.lir_target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let coordinate = ConeCoordinate::new("dev.example", "layout-library", "0.1.0").unwrap();
    let coordinates = [front.coordinate().clone(), coordinate.clone()];
    let (source_graph, _, _) = identities(&hir.hir, &mir.strong, None, &front);
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&source_graph, &coordinates).unwrap();
    let dependency_layouts = match (provider_exports, layout_provider) {
        (Some((_, layout)), Some(provider)) => physical::select(&mir.strong, layout, provider),
        _ => lir::StrongProductionDependencySelectionV2::empty(
            mir.strong.module().cone,
            target.lir_target(),
        )
        .unwrap(),
    };
    let string = scoop_lir_lower::RuntimeStringDescriptor::External(string);
    let lir = match layout_provider {
        Some(_) => scoop_lir_lower::lower_with_layout_dependencies(
            &mir.strong,
            string,
            &selected,
            target.lir_target(),
            &dependency_layouts,
            &diagnostics,
        ),
        None => scoop_lir_lower::lower_with_diagnostics(
            &mir.strong,
            string,
            &selected,
            target.lir_target(),
            &diagnostics,
        ),
    }
    .unwrap();
    let (graph, core_lir, core_hir) = identities(&hir.hir, &mir.strong, Some(&lir), &front);
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(&hir.hir).unwrap(),
    )
    .unwrap();
    let core = closure.direct_provider(ConeIdentity::CORE).unwrap();
    let source = scoop_hir_lower::produce_cross_cone_type_semantics(
        &hir.hir,
        hir::SharedTypeMetadataV1 {
            provider: mir.strong.module().cone,
            identities: &graph,
            foundation: &foundation,
            public: &hir.cross_cone_section,
        },
        &[hir::SharedTypeMetadataV1 {
            provider: core.identity(),
            identities: &graph,
            foundation: &core_hir,
            public: core.production().hir_interface(),
        }],
    )
    .unwrap();
    let types = match provider_exports {
        Some((provider, _)) => provider.types().clone(),
        None => dependencies::mir_types(&mir.strong, &graph),
    };
    let mir_callables = provider_exports
        .map(|(provider, _)| provider.callables())
        .into_iter()
        .collect::<Vec<_>>();
    let dispatch = provider_exports
        .map(|(provider, _)| provider.dispatch())
        .into_iter()
        .collect::<Vec<_>>();
    let input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
        hir: &hir.hir,
        public: &hir.cross_cone_section,
        source: &source,
        mir: &mir.strong,
        ordinary: &mir.public,
        nominal_classifier: &hir.nominal_classifier,
        identities: &graph,
    };
    let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
        types: &[&types],
        callables: &mir_callables,
        dispatch: &dispatch,
    };
    let bridge = scoop_mir_lower::lower_type_bridge_exports(input, dependencies).unwrap();
    let projected =
        scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(input, dependencies).unwrap();
    bridge
        .validate_sources(mir.strong.module().cone, &graph, &projected)
        .unwrap();
    let uses =
        mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(&projected).unwrap();
    assert!(!uses.is_empty());
    assert!(
        uses.iter()
            .all(|usage| usage.provider() == ConeIdentity::CORE
                && matches!(
                    usage.target(),
                    mir::MirTypeBridgeTargetV1::Type(_)
                        | mir::MirTypeBridgeTargetV1::ShapeSupport(_)
                ))
    );
    assert_eq!(
        mir::MirTypeBridgeSectionSourceAuthorityV1::local_initialization_units(&projected)
            .unwrap()
            .len(),
        mir.strong.materialization().initialization_roots().len(),
    );
    let mir_input = input;
    let layouts = match provider_exports {
        Some((_, provider)) => provider.layouts().clone(),
        None => dependencies::layouts(&types, &core_lir, &graph, target.lir_target()),
    };
    let lir_callables = provider_exports
        .map(|(_, provider)| provider.callables())
        .into_iter()
        .collect::<Vec<_>>();
    let registration = lir
        .build_production_section_v2(
            coordinate.clone(),
            &[scoop_identity::ConeIdentity::CORE],
            lir::EntryProductionSourceV1::Library,
            &dependency_layouts,
            &[],
        )
        .unwrap();
    let input = scoop_lir_lower::LayoutAbiExportInputV1 {
        mir: &mir.strong,
        lir: &lir,
        bridge: &bridge,
        registration: &registration,
        identities: &graph,
        coordinates: &coordinates,
    };
    let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
        layouts: &[&layouts],
        callables: &lir_callables,
    };
    if bridge.initialization_uses().records().is_empty() {
        source_contracts::check(input, dependencies, &projected);
    } else {
        // The baseline has no selected descriptors or external registration edges.
        let error = scoop_lir_lower::lower_layout_abi_dependencies(
            input,
            dependencies,
            &scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap(),
            mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(&projected)
                .unwrap(),
            &[],
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            scoop_lir_lower::LayoutAbiDependencyLoweringError::InitializationEdges(error)
                if matches!(*error, mir::MirObjectBridgeError::InitializationDependencyInventory)
        ));
    }
    shared_ordinary::check_dependency_uses(
        hir::SharedTypeMetadataV1 {
            provider: input.mir.module().cone,
            identities: &graph,
            foundation: &foundation,
            public: &hir.cross_cone_section,
        },
        &mir.public,
        input,
        dependencies,
        core.production().lir_cross_cone(),
        hir::SharedTypeMetadataV1 {
            provider: core.identity(),
            identities: &graph,
            foundation: &core_hir,
            public: core.production().hir_interface(),
        },
        &core_lir,
    );
    let owners = request
        .dependencies()
        .closure
        .dependency_symbol_owners()
        .cloned()
        .collect::<Vec<_>>();
    run(mir_input, &projected, input, dependencies, &owners);
}
