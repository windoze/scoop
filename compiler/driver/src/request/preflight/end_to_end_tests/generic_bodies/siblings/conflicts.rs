use super::*;

#[test]
fn actual_duplicate_generic_members_accept_different_optimization() {
    let target = resolved_target().expect("ODR validation requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-odr-siblings");
    let coordinate = ConeCoordinate::new("dev.example", "odr-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        coordinate.name(),
        "library",
        &std::fs::read_to_string(fixtures.join("conflict-provider-left.scoop")).unwrap(),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    let provider_bytes = std::fs::read(provider.artifact().path()).unwrap();
    std::fs::remove_dir_all(provider_root).unwrap();
    let mut closures = Vec::new();
    let mut identities = Vec::new();
    for (name, optimization) in [
        ("left", scoop_lir::OptimizationMode::Debug),
        ("right", scoop_lir::OptimizationMode::Release),
    ] {
        let root = sysroot.path().join(name);
        write_manifest_cone(
            &root,
            "dev.example",
            name,
            "library",
            &std::fs::read_to_string(fixtures.join("conflict-consumer.scoop")).unwrap(),
        );
        write_dependency_manifest(&root, name, &[&coordinate]);
        let consumer = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .with_optimization(optimization)
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let summary = consumer.artifact().summary();
        let identity = summary.coordinate().identity().unwrap();
        let mut direct = summary
            .direct_dependencies()
            .iter()
            .map(scoop_slib::DependencyRecord::identity)
            .collect::<Vec<_>>();
        direct.sort_unstable();
        let consumer_bytes = std::fs::read(consumer.artifact().path()).unwrap();
        closures.push(
            scoop_slib::read_cross_cone_layout_artifact_closure(
                scoop_slib::CrossConeArtifactClosureInput::completed(
                    identity,
                    target.lir_target_selection(),
                    direct,
                    vec![&core_bytes, &provider_bytes],
                    &consumer_bytes,
                ),
                target.c_bridge_toolchain().profile(),
            )
            .unwrap(),
        );
        identities.push(identity);
    }
    let first = closures[0].artifact(identities[0]).unwrap();
    let second = closures[1].artifact(identities[1]).unwrap();
    let first_bodies = first
        .1
        .final_objects()
        .runtime_images()
        .fingerprint()
        .registrations()
        .callables()
        .fingerprints();
    let second_bodies = second
        .1
        .final_objects()
        .runtime_images()
        .fingerprint()
        .registrations()
        .callables()
        .fingerprints();
    assert!(
        first_bodies.iter().any(|left| second_bodies
            .iter()
            .any(|right| left.body() == right.body()
                && left.body_definition() != right.body_definition())),
        "machine optimization must change a shared body's actual object"
    );
    let first = (first.0.identity_graph(), first.1);
    let second = (second.0.identity_graph(), second.1);
    let merged = scoop_slib::merge_cross_cone_odr_definitions([first, second]).unwrap();
    assert!(
        merged
            .members()
            .any(|member| member.candidates().count() == 2)
    );
    assert!(matches!(
        scoop_slib::merge_cross_cone_odr_definitions([first, first]),
        Err(scoop_slib::OdrDefinitionMergeError::DuplicateArtifact(provider))
            if provider == identities[0]
    ));
}
