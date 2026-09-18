use super::*;

#[test]
fn formal_pipeline_calls_a_direct_dependency_through_the_cross_cone_artifact_closure() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "m23-provider",
        "library",
        "package dependency.api\n\npublic fun run(value: Int): Int = value\n",
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("artifacts/provider.slib"),
    );
    let provider_coordinate = ConeCoordinate::new("dev.example", "m23-provider", "0.1.0").unwrap();

    let consumer_root = sysroot.path().join("consumer");
    write_manifest_cone(
        &consumer_root,
        "dev.example",
        "m23-consumer",
        "library",
        "package consumer\n\nimport dependency.api.run\n\npublic fun invoke(): Int = run(41)\n",
    );
    write_dependency_manifest(&consumer_root, "m23-consumer", &[&provider_coordinate]);
    let consumer = build_manifest_request(
        sysroot.path(),
        &target,
        &consumer_root,
        &sysroot.path().join("artifacts/consumer.slib"),
        vec![provider.artifact().path().to_path_buf()],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap();

    let mut dependencies = vec![ConeIdentity::CORE, provider_coordinate.identity().unwrap()];
    dependencies.sort_unstable();
    assert_graph_dependencies(&consumer, &target, &dependencies);
}
