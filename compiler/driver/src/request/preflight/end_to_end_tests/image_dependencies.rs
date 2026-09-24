use super::*;
use std::fmt::Write;

#[test]
fn published_image_preserves_all_direct_dependencies_in_both_views() {
    let target = resolved_target().expect("image dependency publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-image-dependencies");
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
    .build_and_publish(DecodeLimits::default())
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
        DecodeLimits::default(),
        target.c_bridge_toolchain().profile(),
        &mut scoop_identity::SemanticIdentitySession::new(),
    )
    .unwrap();
    let compile_image = closure
        .current_compile()
        .production()
        .lir_strong()
        .image_plan();
    let link_image = closure.current_link().production().lir().image_plan();
    assert_eq!(compile_image, link_image);
    assert_eq!(compile_image.dependencies(), dependencies);
    let selected = closure
        .current_compile()
        .production()
        .lir_cross_cone()
        .selected();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].provider(), coordinates[0].identity().unwrap());

    let mut snapshot = String::new();
    for (name, provider) in [
        ("core", ConeIdentity::CORE),
        ("provider", coordinates[0].identity().unwrap()),
        ("unused", coordinates[1].identity().unwrap()),
    ] {
        let image = closure
            .semantic()
            .direct_provider(provider)
            .unwrap()
            .production()
            .lir_strong()
            .image_plan();
        writeln!(snapshot, "image {name}").unwrap();
        for dependency in image.dependencies() {
            writeln!(snapshot, "  dependency {dependency}").unwrap();
        }
    }
    writeln!(snapshot, "image consumer").unwrap();
    for dependency in compile_image.dependencies() {
        writeln!(snapshot, "  dependency {dependency}").unwrap();
    }
    let golden = fixtures.join("image.snap");
    if std::env::var_os("SCOOP_UPDATE_IMAGE_DEPENDENCY_SNAPSHOTS").is_some() {
        std::fs::write(&golden, &snapshot).unwrap();
    }
    assert_eq!(snapshot, std::fs::read_to_string(golden).unwrap());
}
