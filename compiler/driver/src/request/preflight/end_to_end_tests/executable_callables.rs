use super::*;

mod validation;

#[test]
fn published_machine_callables_follow_actual_bodies_and_default_evaluation() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let fixtures =
        crate::workspace_root().join("tests/fixtures/m23-executable-dependency-callables");
    let source =
        |case: &str| std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let provider_root = sysroot.path().join("provider");
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "executable-provider", "0.1.0").unwrap();
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "executable-provider",
        "library",
        &source("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    let facade_root = sysroot.path().join("facade");
    let facade_coordinate =
        ConeCoordinate::new("dev.example", "executable-facade", "0.1.0").unwrap();
    write_manifest_cone(
        &facade_root,
        "dev.example",
        "executable-facade",
        "library",
        &source("facade"),
    );
    write_dependency_manifest(&facade_root, "executable-facade", &[&provider_coordinate]);
    let facade = build_manifest_request(
        sysroot.path(),
        &target,
        &facade_root,
        &sysroot.path().join("output/facade.slib"),
        vec![provider.artifact().path().to_path_buf()],
        vec![],
    )
    .build_and_publish()
    .unwrap();
    let dependency_bytes = [&core, &provider, &facade]
        .map(|artifact| std::fs::read(artifact.artifact().path()).unwrap());
    for (case, selected) in [
        ("standalone", 0),
        ("evaluated", 1),
        ("routes", 1),
        ("uninstantiated", 0),
        ("metadata", 3),
    ] {
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source(case));
        write_dependency_manifest(&root, case, &[&provider_coordinate, &facade_coordinate]);
        let request = || {
            build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join(format!("output/{case}.slib")),
                vec![
                    provider.artifact().path().to_path_buf(),
                    facade.artifact().path().to_path_buf(),
                ],
                vec![],
            )
        };
        validation::check_machine_input(request(), selected);
        let published = request().build_and_publish().unwrap();
        let bytes = std::fs::read(published.artifact().path()).unwrap();
        let identity = ConeCoordinate::new("dev.example", case, "0.1.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut direct = vec![
            ConeIdentity::CORE,
            provider_coordinate.identity().unwrap(),
            facade_coordinate.identity().unwrap(),
        ];
        direct.sort_unstable();
        let mut session = scoop_identity::SemanticIdentitySession::new();
        let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
            identity,
            target.lir_target_selection(),
            direct,
            dependency_bytes.iter().map(Vec::as_slice).collect(),
            &bytes,
            target.c_bridge_toolchain().profile(),
            &mut session,
        )
        .unwrap();
        let production = closure.current_compile().production();
        assert_eq!(production.mir_cross_cone().selected().len(), selected);
        assert_eq!(production.lir_cross_cone().selected().len(), selected);
        assert_eq!(
            closure
                .current_link()
                .symbol_uses()
                .undefined_partitions()
                .cross_cone()
                .semantic_imports()
                .imports()
                .len(),
            selected
        );
        let references = production.hir_interface().external_references().records();
        let concrete = references
            .iter()
            .filter(|record| {
                record
                    .roles()
                    .contains(scoop_hir::ExternalHirReferenceRoleV1::ConcreteSelectedUse)
            })
            .count();
        assert_eq!(concrete, selected);
        if case == "metadata" {
            let sites = references
                .iter()
                .flat_map(|record| record.call_sites().records())
                .collect::<Vec<_>>();
            assert_eq!(sites.len(), 8);
            assert_eq!(
                sites
                    .iter()
                    .filter(|site| site.origin().definition().source().cone() != identity)
                    .count(),
                2
            );
            assert!(
                sites
                    .iter()
                    .all(|site| site.origin().evaluation().source().cone() == identity)
            );
            for site in &sites {
                let expected_routes = if site.origin().definition().source().cone() == identity {
                    1
                } else {
                    2
                };
                assert_eq!(site.witness_indices().len(), expected_routes);
            }
            assert!(
                sites.iter().any(|site| site.arguments().len() == 2
                    && site.arguments()[0] == site.arguments()[1])
            );
        }
        let defaults = production
            .hir_interface()
            .default_templates()
            .records()
            .len();
        assert_eq!(defaults, usize::from(case != "uninstantiated"));
        let mut dump = format!(
            "default_templates={defaults}\nhir_concrete={concrete}\nmir_selected={selected}\nlir_selected={selected}\nlink_imports={selected}\n"
        );
        for reference in references {
            for site in reference.call_sites().records() {
                dump.push_str(&format!(
                    "call={:?} arguments={} witnesses={:?} foreign_definition={}\n",
                    site.position(),
                    site.arguments().len(),
                    site.witness_indices(),
                    site.origin().definition().source().cone() != identity,
                ));
            }
        }
        if let Some(directory) = std::env::var_os("SCOOP_EXECUTABLE_CALLABLE_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(Path::new(&directory).join(format!("{case}.snap")), dump).unwrap();
        } else {
            assert_eq!(
                dump,
                std::fs::read_to_string(fixtures.join(format!("{case}.snap"))).unwrap()
            );
        }
    }
}
