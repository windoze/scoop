use super::super::imported_classes::runtime;
use super::*;

mod combinations;
mod negatives;
mod siblings;

#[test]
fn generic_delegated_properties_republish_and_execute_from_artifacts() {
    let target = resolved_target().expect("generic delegate publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-delegate-consumption");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-delegate-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &source("provider"),
    );
    let provider = build_manifest(
        sysroot.path(),
        &target,
        &provider_root,
        &sysroot.path().join("output/provider.slib"),
    );
    std::fs::rename(
        provider_root.join("src"),
        provider_root.join("unused-source"),
    )
    .unwrap();

    let consumer_coordinate =
        ConeCoordinate::new("dev.example", "generic-delegate-consumer", "0.1.0").unwrap();
    let consumer_root = sysroot.path().join("consumer");
    write_manifest_cone(
        &consumer_root,
        "dev.example",
        consumer_coordinate.name(),
        "library",
        &source("consumer"),
    );
    write_dependency_manifest(
        &consumer_root,
        consumer_coordinate.name(),
        &[&provider_coordinate],
    );
    let mut outputs = Vec::new();
    for (kind, stage) in [
        (StageDumpKind::Hir, "hir"),
        (StageDumpKind::Mir, "mir"),
        (StageDumpKind::Lir, "lir"),
    ] {
        let mut request = build_manifest_request(
            sysroot.path(),
            &target,
            &consumer_root,
            &sysroot.path().join("output/consumer.slib"),
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        );
        request.emit = StageDumpPolicy::Stage(kind);
        let output = request
            .build_and_publish()
            .unwrap_or_else(|error| panic!("generic delegate {stage}: {error:?}"));
        let snapshot = fixtures.join(format!("consumer.{stage}.snap"));
        let actual = output.emitted_dump().unwrap().text();
        if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
        outputs.push(output);
    }
    let consumer = outputs.last().unwrap();
    std::fs::rename(
        consumer_root.join("src"),
        consumer_root.join("unused-source"),
    )
    .unwrap();
    let downstream_root = sysroot.path().join("downstream");
    write_manifest_cone(
        &downstream_root,
        "dev.example",
        "generic-delegate-downstream",
        "library",
        &source("downstream"),
    );
    write_dependency_manifest(
        &downstream_root,
        "generic-delegate-downstream",
        &[&consumer_coordinate],
    );
    let downstream = build_manifest_request(
        sysroot.path(),
        &target,
        &downstream_root,
        &sysroot.path().join("output/downstream.slib"),
        vec![consumer.artifact().path().to_path_buf()],
        vec![provider.artifact().path().to_path_buf()],
    )
    .build_and_publish()
    .unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let closure = runtime::check(
        &target,
        &[&core, &provider, consumer, &downstream],
        &runtime,
        &crate::workspace_root().join("tests/fixtures/m23-imported-classes"),
        &sysroot.path().join("run"),
        "generic-delegate",
    );
    let (sections, _) = closure
        .artifact(consumer_coordinate.identity().unwrap())
        .unwrap();
    let units = sections
        .lir_strong_production()
        .registration_production()
        .initialization_units()
        .registrations();
    assert_eq!(units.len(), 3);
    for unit in units {
        assert_eq!(
            unit.semantic().schedule(),
            scoop_lir::StrongInitializationSchedulePlanV1::LazyAccess
        );
        assert_eq!(
            unit.registration_symbol().linkage(),
            scoop_identity::LinkageClass::OdrWeak
        );
    }
}
