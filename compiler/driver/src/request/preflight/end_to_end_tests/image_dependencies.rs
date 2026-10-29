use super::*;

#[test]
fn published_image_preserves_all_direct_dependencies_in_both_views() {
    let target = resolved_target().expect("image dependency publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/language/image-dependencies");
    let mut artifacts = vec![core];
    let mut coordinates = Vec::new();
    for name in ["provider", "unused"] {
        let root = sysroot.path().join(name);
        let cone_name = format!("image-{name}");
        let source = std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
        write_manifest_cone(&root, "dev.example", &cone_name, "library", &source);
        artifacts.push(build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
        ));
        coordinates.push(ConeCoordinate::new("dev.example", &cone_name, "0.1.0").unwrap());
    }
    let consumer_root = sysroot.path().join("consumer");
    let source = std::fs::read_to_string(fixtures.join("consumer.scoop")).unwrap();
    write_manifest_cone(
        &consumer_root,
        "dev.example",
        "image-consumer",
        "library",
        &source,
    );
    write_dependency_manifest(
        &consumer_root,
        "image-consumer",
        &coordinates.iter().collect::<Vec<_>>(),
    );
    let consumer = build_manifest_request(
        sysroot.path(),
        &target,
        &consumer_root,
        &sysroot.path().join("output/consumer.slib"),
        artifacts[1..]
            .iter()
            .map(|artifact| artifact.artifact().path().to_path_buf())
            .collect(),
        vec![],
    )
    .build_and_publish()
    .unwrap();
    let mut dependencies = vec![ConeIdentity::CORE];
    dependencies.extend(
        coordinates
            .iter()
            .map(|coordinate| coordinate.identity().unwrap()),
    );
    dependencies.sort_unstable();
    assert_graph_dependencies(&consumer, &target, &dependencies);
    let provider_bytes: Vec<_> = artifacts
        .iter()
        .map(|artifact| std::fs::read(artifact.artifact().path()).unwrap())
        .collect();
    let bytes = std::fs::read(consumer.artifact().path()).unwrap();
    let identity = ConeCoordinate::new("dev.example", "image-consumer", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();
    let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
        identity,
        target.lir_target_selection(),
        dependencies.clone(),
        provider_bytes.iter().map(Vec::as_slice).collect(),
        &bytes,
        &mut scoop_identity::SemanticIdentitySession::new(),
    )
    .unwrap();
    assert_eq!(
        closure
            .current_link()
            .direct_dependencies()
            .iter()
            .map(|dependency| dependency.identity())
            .collect::<Vec<_>>(),
        dependencies,
    );
    let selected = closure
        .current_compile()
        .production()
        .lir_cross_cone()
        .selected()
        .iter()
        .filter(|selection| selection.provider() != ConeIdentity::CORE)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].provider(), coordinates[0].identity().unwrap());

    for provider in [
        ConeIdentity::CORE,
        coordinates[0].identity().unwrap(),
        coordinates[1].identity().unwrap(),
    ] {
        let semantic = closure.semantic().direct_provider(provider).unwrap();
        let manifest = semantic.production().layout().manifest();
        let expected = if provider == ConeIdentity::CORE {
            vec![]
        } else {
            vec![ConeIdentity::CORE]
        };
        assert_eq!(
            manifest
                .direct_dependencies()
                .iter()
                .map(|dependency| dependency.identity())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
