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
        &[mir::MirTypeBridgeDependencyV1],
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
        |mir, projection, lir, dependencies, _, _| {
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
        &[mir::MirTypeBridgeDependencyV1],
        scoop_lir_lower::LayoutAbiExportInputV1<'_>,
        scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
        &[scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1],
        &[lir::ExternalShapeLinkImportV1],
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
        Default::default(),
    )
    .unwrap();
    let selected = closure
        .project_dependency_callables_to_mir(&hir.hir)
        .unwrap();
    let mir = hir.machine_input().lower_selected_mir(selected).unwrap();
    let selected = closure
        .project_dependency_callables_to_lir(mir.strong.selected_callables())
        .unwrap();
    let core_read = scoop_slib::read_cross_cone_layout_artifact_closure(
        scoop_slib::CrossConeArtifactClosureInput::completed(
            ConeIdentity::CORE,
            target.lir_target_selection(),
            vec![],
            vec![],
            core_bytes,
        ),
        target.c_bridge_toolchain().profile(),
    )
    .unwrap();
    let (front, _) = core_read.artifact(ConeIdentity::CORE).unwrap();
    let coordinate = ConeCoordinate::new("dev.example", "layout-library", "0.1.0").unwrap();
    let coordinates = [front.coordinate().clone(), coordinate.clone()];
    let (source_graph, _, _) = identities(&hir.hir, &mir.strong, None, front);
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&source_graph, &coordinates).unwrap();
    let string_provider = inputs.protocols().fundamental_types().string().provider();
    let default_dependency = closure
        .layout_dependencies()
        .find(|dependency| dependency.identity() == string_provider)
        .unwrap();
    let default_exports = default_dependency.lir_exports();
    let default_provider = lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
        foundation: default_dependency.lir_foundation(),
        production: default_dependency.lir_strong_production(),
        ordinary: default_dependency.lir_cross_cone_bridge(),
        layouts: default_exports.layouts(),
        callables: default_exports.callables(),
        descriptors: default_exports.descriptors(),
        dispatch: default_exports.dispatch(),
    })
    .unwrap();
    let (layout, provider) = match (provider_exports, layout_provider) {
        (Some((_, layout)), Some(provider)) => (layout, provider),
        _ => (default_exports, &default_provider),
    };
    let dependency_layouts = physical::select(&mir.strong, layout, provider);
    let lir = scoop_lir_lower::lower_with_layout_dependencies(
        &mir.strong,
        &selected,
        target.lir_target(),
        &dependency_layouts,
        &diagnostics,
    )
    .unwrap();
    let (graph, core_lir, core_hir) = identities(&hir.hir, &mir.strong, Some(&lir), front);
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(&hir.hir).unwrap();
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
        None => front.mir_type_bridge().exports().types().clone(),
    };
    let mir_callables = provider_exports
        .map(|(provider, _)| provider.callables())
        .or(Some(front.mir_type_bridge().exports().callables()))
        .into_iter()
        .collect::<Vec<_>>();
    let dispatch = provider_exports
        .map(|(provider, _)| provider.dispatch())
        .or(Some(front.mir_type_bridge().exports().dispatch()))
        .into_iter()
        .collect::<Vec<_>>();
    let input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
        hir: &hir.hir,
        public: &hir.cross_cone_section,
        source: &source,
        mir: &mir.strong,
        ordinary: &mir.public,
        dependency_objects: mir.strong.selected_callables().objects(),
        identities: &graph,
    };
    let dependencies = scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
        types: &[&types],
        callables: &mir_callables,
        direct_callables: &[front.mir_cross_cone_bridge()],
        dispatch: &dispatch,
    };
    let bridge = scoop_mir_lower::lower_type_bridge_exports(input, dependencies).unwrap();
    let projected = scoop_mir_lower::lower_type_bridge_dependencies(input).unwrap();
    let uses = &projected;
    assert!(!uses.is_empty());
    assert!(
        uses.iter()
            .all(|usage| usage.provider() == ConeIdentity::CORE
                && matches!(
                    usage.target(),
                    mir::MirTypeBridgeTargetV1::Type(_)
                        | mir::MirTypeBridgeTargetV1::ShapeSupport(_)
                        | mir::MirTypeBridgeTargetV1::Callable(_)
                ))
    );
    assert_eq!(
        scoop_mir_lower::lower_type_bridge_initialization_units(&mir.strong)
            .unwrap()
            .len(),
        mir.strong.materialization().initialization_roots().len(),
    );
    let mir_input = input;
    let layouts = match provider_exports {
        Some((_, provider)) => provider.layouts().clone(),
        None => front.lir_exports().layouts().clone(),
    };
    let lir_callables = provider_exports
        .map(|(_, provider)| provider.callables())
        .or(Some(front.lir_exports().callables()))
        .into_iter()
        .collect::<Vec<_>>();
    let registration = lir
        .build_production_section_v2(
            coordinate.clone(),
            &[scoop_identity::ConeIdentity::CORE],
            lir::EntryProductionSourceV1::Library,
            &[],
        )
        .unwrap();
    let ordinary =
        scoop_lir_lower::lower_cross_cone_bridge_section(&mir.strong, &mir.public, &lir).unwrap();
    let input = scoop_lir_lower::LayoutAbiExportInputV1 {
        mir: &mir.strong,
        lir: &lir,
        bridge: &bridge,
        ordinary: &ordinary,
        registration: &registration,
        identities: &graph,
        coordinates: &coordinates,
    };
    let dependencies = scoop_lir_lower::LayoutAbiExportDependenciesV1 {
        layouts: &[&layouts],
        callables: &lir_callables,
        direct_callables: &[layout.direct_callables()],
    };
    if bridge.initialization_uses().records().is_empty() {
        source_contracts::check(
            input,
            dependencies,
            &projected,
            dependency_layouts.physical_imports().records(),
        );
    } else {
        // The baseline has no selected descriptors or external registration edges.
        let error = scoop_lir_lower::lower_layout_abi_dependencies(
            input,
            dependencies,
            &scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap(),
            &projected,
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
    run(
        mir_input,
        &projected,
        input,
        dependencies,
        &owners,
        dependency_layouts.physical_imports().records(),
    );
}
