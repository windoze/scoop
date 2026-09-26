use super::*;

mod artifact;
mod contracts;
mod layout_section;
mod section;
mod shared_accessors;

#[test]
fn ordinary_library_exports_members_with_available_machine_signatures() {
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
            let result = scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap();
            assertions::actual(input, &result);
            let names = result
                .callables()
                .records()
                .iter()
                .map(|callable| callable.target())
                .chain(
                    result
                        .direct_callables()
                        .exports()
                        .iter()
                        .map(|callable| callable.target()),
                )
                .map(|target| assertions::callable_name(input, target))
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
    check_core_layout_exports(&["base", "standalone", "combined"]);
}

#[test]
fn actual_layout_artifacts_retain_alias_chains_and_combined_member_signatures() {
    check_core_layout_exports(&["shared-aliases-standalone", "shared-aliases-combined"]);
}

#[test]
fn shared_mir_types_replay_standalone_and_combined_source_policies() {
    check_core_layout_exports(&["shared-mir-standalone", "shared-mir-combined"]);
}

#[test]
fn shared_accessor_forms_select_only_actual_source_machine_bodies() {
    check_core_layout_exports(&["shared-accessors-standalone", "shared-accessors-combined"]);
}

#[test]
fn shared_source_callable_inventory_replays_bodies_and_abstract_overrides() {
    check_core_layout_exports(&["shared-callables-standalone", "shared-callables-combined"]);
}

#[test]
fn shared_constructor_inventory_replays_primary_secondary_and_class_initializers() {
    check_core_layout_exports(&[
        "shared-constructors-standalone",
        "shared-constructors-combined",
    ]);
}

#[test]
fn heap_zst_fields_produce_valid_layout_objects_and_shared_metadata() {
    check_core_layout_exports(&["heap-zst-storage"]);
}

#[test]
fn shared_object_inventory_replays_singletons_companions_and_initialization_entries() {
    check_core_layout_exports(&["shared-objects-standalone", "shared-objects-combined"]);
}

#[test]
fn shared_dispatch_replays_slot_order_targets_reabstraction_and_boxing() {
    check_core_layout_exports(&["shared-dispatch-standalone", "shared-dispatch-combined"]);
}

#[test]
fn shared_equality_replays_materialized_source_applications_and_default_only_keys() {
    check_core_layout_exports(&["shared-equality-standalone", "shared-equality-combined"]);
}

#[test]
fn shared_initialization_units_replay_source_keys_and_complete_strong_pairs() {
    check_core_layout_exports(&["shared-units-standalone", "shared-units-combined"]);
}

#[test]
fn shared_lir_layouts_replay_recursive_references_zst_and_base_prefixes() {
    check_core_layout_exports(&["shared-layouts-standalone", "shared-layouts-combined"]);
}

#[test]
fn shared_lir_callable_abis_replay_zst_indirect_results_and_boxing_adjustments() {
    check_core_layout_exports(&["shared-abi-standalone", "shared-abi-combined"]);
}

#[test]
fn shared_lir_dispatch_replays_complete_tables_and_actual_callable_abis() {
    check_core_layout_exports(&[
        "shared-lir-dispatch-standalone",
        "shared-lir-dispatch-combined",
    ]);
}

#[test]
fn shared_lir_descriptors_replay_ancestry_scans_and_registration_from_constituents() {
    check_core_layout_exports(&["shared-td-standalone", "shared-td-combined"]);
}

#[test]
fn shared_lir_shape_support_replays_finite_helpers_from_checked_mir_roots() {
    check_core_layout_exports(&["shared-shapes-standalone", "shared-shapes-combined"]);
}

#[test]
fn shared_ordinary_lir_bridges_replay_source_gc_and_layout_abis() {
    check_core_layout_exports(&["shared-ordinary-standalone", "shared-ordinary-combined"]);
}

#[test]
fn shared_initialization_abi_replays_the_protocol_role_and_complete_layout_contract() {
    check_core_layout_exports(&["shared-init-abi-standalone", "shared-init-abi-combined"]);
}

#[test]
fn shared_strong_digests_replay_complete_runtime_registration_roles() {
    check_core_layout_exports(&["shared-digests-standalone", "shared-digests-combined"]);
}

#[test]
fn shared_strong_reader_replays_complete_sections_from_actual_artifact_bytes() {
    check_core_layout_exports(&["shared-production-standalone", "shared-production-combined"]);
}

#[test]
fn property_initialization_uses_close_source_mir_lir_and_both_artifact_views() {
    check_core_layout_exports(&["property-initialization-provider"]);
}

fn check_core_layout_exports(names: &[&str]) {
    let target = resolved_target().expect("core layout exports require a host target");
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-core-layout-exports");
    for &name in names {
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
            ExplicitDependencyInputs::new(vec![], vec![]),
            TrustedCoreInput::BootstrapSelf,
            target.clone(),
            SlibOutputDestination::new(directory.path().join("core.slib")).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap()
        .load_preflight()
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
        let mir = hir
            .machine_input()
            .lower_selected_mir(mir::SelectedExternalMirSet::empty(ConeIdentity::CORE))
            .unwrap();
        let coordinates = [ConeCoordinate::reserved_core()];
        let source_graph = identity_graph(&hir.hir, &mir.strong, None);
        let diagnostics =
            scoop_identity::ExactTypeDiagnosticCatalog::try_new(&source_graph, &coordinates)
                .unwrap();
        let lir = scoop_lir_lower::lower_with_diagnostics(
            &mir.strong,
            &[],
            &lir::SelectedExternalLirSet::empty(ConeIdentity::CORE),
            target.lir_target(),
            &diagnostics,
        )
        .unwrap();
        let identities = identity_graph(&hir.hir, &mir.strong, Some(&lir));
        let foundation = hir::OdrFreeHirFoundation::try_new(
            hir::CanonicalHirFoundation::from_type_semantics_output(&hir.hir).unwrap(),
        )
        .unwrap();
        let source = scoop_hir_lower::produce_cross_cone_type_semantics(
            &hir.hir,
            hir::SharedTypeMetadataV1 {
                provider: hir.hir.output().export.cone,
                identities: &identities,
                foundation: &foundation,
                public: &hir.cross_cone_section,
            },
            &[],
        )
        .unwrap();
        let mir_input = scoop_mir_lower::MirTypeBridgeExportInputV1 {
            hir: &hir.hir,
            public: &hir.cross_cone_section,
            source: &source,
            mir: &mir.strong,
            ordinary: &mir.public,
            dependency_objects: mir.selected_callables.objects(),
            identities: &identities,
        };
        let bridge = scoop_mir_lower::lower_type_bridge_exports(
            mir_input,
            scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
                types: &[],
                callables: &[],
                direct_callables: &[],
                dispatch: &[],
            },
        )
        .unwrap_or_else(|error| panic!("{name} MIR exports: {error}"));
        shared_accessors::check(name, mir_input, &bridge);
        let registration = lir
            .build_production_section_v2(
                coordinates[0].clone(),
                &[],
                lir::EntryProductionSourceV1::Library,
                &[],
            )
            .unwrap();
        let ordinary =
            scoop_lir_lower::lower_cross_cone_bridge_section(&mir.strong, &mir.public, &lir)
                .unwrap();
        let input = scoop_lir_lower::LayoutAbiExportInputV1 {
            mir: &mir.strong,
            lir: &lir,
            bridge: &bridge,
            ordinary: &ordinary,
            registration: &registration,
            identities: &identities,
            coordinates: &coordinates,
        };
        let result = scoop_lir_lower::lower_layout_abi_exports(
            input,
            scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
        )
        .unwrap_or_else(|error| panic!("{name} LIR exports: {error}"));
        shared_layouts::check(
            input,
            scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
            &result,
        );
        if name.starts_with("shared-layouts-") {
            shared_layouts::probe(name, input, &result);
        }
        shared_abis::check(
            input,
            scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
            &result,
        );
        if name.starts_with("shared-abi-") {
            shared_abis::probe(input, &result);
        }
        shared_dispatch::check(
            input,
            scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
            &result,
        );
        if name.starts_with("shared-lir-dispatch-") {
            shared_dispatch::probe(input, &result);
        }
        shared_descriptors::check(input, &result);
        if name.starts_with("shared-td-") {
            shared_descriptors::probe(input, &result);
        }
        shared_shapes::check(input, &result);
        if name.starts_with("shared-shapes-") {
            shared_shapes::probe(input, &result);
        }
        let mut dump = contracts::check(name, &hir, &source, input, &result);
        assertions::contents(input, &result);
        if matches!(name, "standalone" | "combined") {
            assertions::zero_sized_abi(&result);
        }
        dump.push_str(&assertions::dump(input, &result));
        let snapshot = fixtures.join(format!("{name}.snap"));
        if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
            std::fs::write(&snapshot, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
        let layout_section = layout_section::check(mir_input, input, result);
        let production = registration.validate_layout_abi(&layout_section).unwrap();
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
    let hir = hir::CanonicalHirFoundation::from_type_semantics_output(hir).unwrap();
    let mir = mir.foundation().as_canonical();
    let lir = lir.map(|lir| lir.foundation().as_canonical());
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    if let Some(lir) = &lir {
        lir.register_identities(&mut pending).unwrap();
    }
    pending.finish().unwrap()
}
