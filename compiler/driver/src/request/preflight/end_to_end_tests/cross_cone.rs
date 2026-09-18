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

#[test]
fn chained_reexport_resolves_every_machine_use_to_the_terminal_provider() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);

    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "m23-terminal-provider",
        "library",
        r#"package dependency.api

public typealias Number = Int
public const val OFFSET: Int = 2

public fun helper(value: Int): Int = value
public fun add(value: Number, amount: Int = helper(OFFSET)): Int = value

public val answer: Int
    get() = add(40)

public fun Int.bump(): Int = this
"#,
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("artifacts/terminal-provider.slib"),
    );
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "m23-terminal-provider", "0.1.0").unwrap();
    let provider_identity = provider_coordinate.identity().unwrap();

    let facade_root = sysroot.path().join("facade");
    write_manifest_cone(
        &facade_root,
        "dev.example",
        "m23-facade",
        "library",
        r#"package facade.api

public import dependency.api.Number
public import dependency.api.OFFSET
public import dependency.api.add
public import dependency.api.answer
public import dependency.api.bump
public import dependency.api.helper
"#,
    );
    write_dependency_manifest(&facade_root, "m23-facade", &[&provider_coordinate]);
    let facade = build_manifest_request(
        sysroot.path(),
        &target,
        &facade_root,
        &sysroot.path().join("artifacts/facade.slib"),
        vec![provider.artifact().path().to_path_buf()],
        Vec::new(),
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap();
    let facade_coordinate = ConeCoordinate::new("dev.example", "m23-facade", "0.1.0").unwrap();
    let facade_identity = facade_coordinate.identity().unwrap();

    let consumer_root = sysroot.path().join("consumer");
    write_manifest_cone(
        &consumer_root,
        "dev.example",
        "m23-reexport-consumer",
        "library",
        r#"package consumer

import facade.api.*

public fun invoke(value: Number): Int {
    val fromDefault = add(value)
    val fromConst = OFFSET
    val fromProperty = answer
    val fromExtension = value.bump()
    return fromDefault
}
"#,
    );
    write_dependency_manifest(
        &consumer_root,
        "m23-reexport-consumer",
        &[&facade_coordinate],
    );
    let consumer = build_manifest_request(
        sysroot.path(),
        &target,
        &consumer_root,
        &sysroot.path().join("artifacts/reexport-consumer.slib"),
        vec![facade.artifact().path().to_path_buf()],
        vec![provider.artifact().path().to_path_buf()],
    )
    .build_and_publish(DecodeLimits::default())
    .unwrap();
    let consumer_identity = ConeCoordinate::new("dev.example", "m23-reexport-consumer", "0.1.0")
        .unwrap()
        .identity()
        .unwrap();

    let mut expected_facade_dependencies = vec![ConeIdentity::CORE, provider_identity];
    expected_facade_dependencies.sort_unstable();
    assert_graph_dependencies(&facade, &target, &expected_facade_dependencies);
    let mut expected_consumer_dependencies = vec![ConeIdentity::CORE, facade_identity];
    expected_consumer_dependencies.sort_unstable();
    assert_graph_dependencies(&consumer, &target, &expected_consumer_dependencies);

    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    let provider_bytes = std::fs::read(provider.artifact().path()).unwrap();
    let facade_bytes = std::fs::read(facade.artifact().path()).unwrap();
    let consumer_bytes = std::fs::read(consumer.artifact().path()).unwrap();
    let mut direct = vec![ConeIdentity::CORE, facade_identity];
    direct.sort_unstable();
    let mut session = scoop_identity::SemanticIdentitySession::new();
    let closure = scoop_slib::validate_completed_cross_cone_artifact_closure(
        consumer_identity,
        target.lir_target_selection(),
        direct,
        vec![&core_bytes, &provider_bytes, &facade_bytes],
        &consumer_bytes,
        DecodeLimits::default(),
        target.c_bridge_toolchain().profile(),
        &mut session,
    )
    .unwrap();

    let facade_production = closure
        .semantic()
        .direct_provider(facade_identity)
        .unwrap()
        .production();
    assert!(facade_production.lir_cross_cone().exports().is_empty());
    assert!(facade_production.lir_cross_cone().selected().is_empty());

    let selected = closure
        .current_compile()
        .production()
        .lir_cross_cone()
        .selected();
    assert_eq!(selected.len(), 4);
    assert!(
        selected
            .iter()
            .all(|selection| selection.provider() == provider_identity)
    );
    let link_imports = closure
        .current_link()
        .cross_cone_link_closure()
        .semantic_imports()
        .imports();
    assert_eq!(link_imports.len(), selected.len());
    assert!(
        link_imports
            .iter()
            .all(|import| import.provider() == provider_identity)
    );
}
