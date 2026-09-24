use super::*;

mod artifact;
mod contracts;
mod layout_section;
mod section;

#[test]
fn ordinary_library_omits_only_source_only_machine_signatures() {
    let target = resolved_target().expect("layout exports require a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-core-layout-exports");
    let source = std::fs::read_to_string(fixtures.join("ordinary.scoop")).unwrap();
    support::with_production(
        sysroot.path(),
        &target,
        &core_bytes,
        &source,
        |input, dependencies| {
            let result =
                scoop_lir_lower::lower_layout_abi_exports(input, dependencies, &mut meter())
                    .unwrap();
            assertions::actual(input, &result);
            let names = result
                .callables()
                .records()
                .iter()
                .map(|callable| assertions::callable_name(input, callable.target()))
                .collect::<Vec<_>>();
            assert!(names.contains(&"Published.ready"));
            assert!(!names.contains(&"Published.deferred"));
            let dump = assertions::dump(input, &result);
            let snapshot = fixtures.join("ordinary.snap");
            if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
                std::fs::write(&snapshot, &dump).unwrap();
            }
            assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
        },
    );
}

#[test]
fn actual_core_sources_produce_closed_mir_and_lir_export_tables() {
    let target = resolved_target().expect("core layout exports require a host target");
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-core-layout-exports");
    for name in ["base", "standalone", "combined"] {
        let directory = tempfile::tempdir().unwrap();
        copy_trusted_core_sources(directory.path());
        let root = directory.path().join("lib/scoop.core");
        std::fs::copy(
            fixtures.join(format!("{name}.scoop")),
            root.join("src/layout_probe.scoop"),
        )
        .unwrap();
        let loaded = SingleConeBuildRequest::new(
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::cone_directory(&root),
            },
            ExplicitDependencyInputs::new(vec![], vec![]).unwrap(),
            TrustedCoreInput::BootstrapSelf,
            target.clone(),
            SlibOutputDestination::new(directory.path().join("core.slib")).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap()
        .load_preflight(DecodeLimits::default())
        .unwrap();
        let request = loaded.validate().unwrap();
        let parsed = request.parse_current_sources().unwrap();
        let world = request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .unwrap();
        let hir = current_hir::CurrentConeHirArtifacts::lower(
            scoop_identity::RequestedConeKind::Library,
            parsed.sources(),
            scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
            &world,
        )
        .unwrap();
        let source = scoop_hir_lower::produce_cross_cone_type_semantics(
            &hir.hir,
            &hir.cross_cone_section,
            &mut meter(),
        )
        .unwrap();
        let mir = hir
            .machine_input()
            .lower_selected_mir(mir::SelectedExternalMirSet::empty(ConeIdentity::CORE))
            .unwrap();
        let coordinates = [ConeCoordinate::reserved_core()];
        let source_graph = identity_graph(&hir.hir, &mir.strong, None);
        let diagnostics = scoop_identity::ExactTypeDiagnosticCatalog::try_new(
            &source_graph,
            &coordinates,
            &mut meter(),
        )
        .unwrap();
        let lir = scoop_lir_lower::lower_with_diagnostics(
            &mir.strong,
            scoop_lir_lower::RuntimeStringDescriptor::Local,
            &lir::SelectedExternalLirSet::empty(ConeIdentity::CORE),
            target.lir_target(),
            &diagnostics,
            &mut meter(),
        )
        .unwrap();
        let identities = identity_graph(&hir.hir, &mir.strong, Some(&lir));
        let mir_input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
            hir: &hir.hir,
            public: &hir.cross_cone_section,
            source: &source,
            mir: &mir.strong,
            ordinary: &mir.public,
            identities: &identities,
        };
        let bridge = scoop_mir_lower::lower_type_bridge_exports(
            mir_input,
            scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                types: &[],
                callables: &[],
                dispatch: &[],
            },
            mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter()).unwrap(),
            &mut meter(),
        )
        .unwrap_or_else(|error| panic!("{name} MIR exports: {error}"));
        let selected = lir::StrongProductionDependencySelectionV2::empty(
            ConeIdentity::CORE,
            target.lir_target(),
            &mut meter(),
        )
        .unwrap();
        let registration = lir
            .build_production_section_v2(
                coordinates[0].clone(),
                &[],
                lir::EntryProductionSourceV1::Library,
                &selected,
                &[],
                &mut meter(),
            )
            .unwrap();
        let input = scoop_lir_lower::LayoutAbiExportInputV1 {
            mir: &mir.strong,
            lir: &lir,
            bridge: &bridge,
            registration: &registration,
            identities: &identities,
            coordinates: &coordinates,
        };
        let result = scoop_lir_lower::lower_layout_abi_exports(
            input,
            scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
            &mut meter(),
        )
        .unwrap_or_else(|error| panic!("{name} LIR exports: {error}"));
        let mut dump = contracts::check(&hir, &source, input, &result);
        assertions::contents(input, &result);
        if name != "base" {
            assertions::zero_sized_abi(&result);
        }
        dump.push_str(&assertions::dump(input, &result));
        let snapshot = fixtures.join(format!("{name}.snap"));
        if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
            std::fs::write(&snapshot, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
        let layout_section = layout_section::check(mir_input, input, result);
        let production = registration
            .validate_layout_abi(&layout_section, &mut meter())
            .unwrap();
        assert_eq!(
            production.type_registrations().registrations().len(),
            lir.module().meta.type_descriptors.len()
        );
        let objects = scoop_codegen::emit_object_set_v2(
            &lir,
            production,
            directory.path(),
            scoop_codegen::ValidatedBackendProfile::from_selection(target.lir_target_selection())
                .unwrap(),
        )
        .unwrap();
        layout_section::snapshot(name, &fixtures, &layout_section, &objects);
        let mir_section = section::check(name, &fixtures, mir_input, bridge);
        artifact::check(
            name,
            directory.path(),
            &target,
            mir_input,
            &lir,
            &mir_section,
            &layout_section,
            objects,
        );
    }
}

fn identity_graph(
    hir: &hir::DependencyHirOutput,
    mir: &mir::SingleConeStrongMirInput,
    lir: Option<&lir::SingleConeStrongLirOutput>,
) -> ValidatedIdentityGraph {
    let hir: hir::DecodedHirFoundation =
        decoded(&hir::CanonicalHirFoundation::from_type_semantics_output(hir).unwrap());
    let mir: mir::DecodedMirFoundation = decoded(mir.foundation().as_canonical());
    let lir: Option<lir::DecodedLirFoundation> =
        lir.map(|lir| decoded(lir.foundation().as_canonical()));
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    if let Some(lir) = &lir {
        lir.register_identities(&mut pending).unwrap();
    }
    hir.resolve_identities(&mut pending).unwrap();
    mir.resolve_identities(&mut pending).unwrap();
    if let Some(lir) = &lir {
        lir.resolve_identities(&mut pending).unwrap();
    }
    pending.finish().unwrap()
}
