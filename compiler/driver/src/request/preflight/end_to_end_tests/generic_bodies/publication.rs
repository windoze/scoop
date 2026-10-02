use super::super::imported_classes::runtime;
use super::*;

#[test]
fn generic_consumers_publish_reusable_artifacts_and_run_with_moving_gc() {
    let target = resolved_target().expect("generic publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-body-consumption");
    let source = |name: &str| {
        std::fs::read_to_string(fixtures.join(format!("machine-{name}.scoop"))).unwrap()
    };
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-machine-provider", "0.1.0").unwrap();
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
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let runtime_fixture = crate::workspace_root().join("tests/fixtures/m23-imported-classes");

    for (case, name) in [
        ("standalone", "generic-machine-standalone"),
        ("combined", "generic-machine-first"),
    ] {
        let coordinate = ConeCoordinate::new("dev.example", name, "0.1.0").unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source(case));
        write_dependency_manifest(&root, name, &[&provider_coordinate]);
        let mut outputs = Vec::new();
        for (kind, stage) in [
            (StageDumpKind::Hir, "hir"),
            (StageDumpKind::Mir, "mir"),
            (StageDumpKind::Lir, "lir"),
        ] {
            let mut request = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &sysroot.path().join(format!("output/{name}.slib")),
                vec![provider.artifact().path().to_path_buf()],
                Vec::new(),
            );
            request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            assert_eq!(
                output.emitted_dumps().first().unwrap().text(),
                std::fs::read_to_string(fixtures.join(format!("machine-{case}.{stage}.snap")))
                    .unwrap(),
                "{case} {stage}",
            );
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("generic-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source(&format!("downstream-{case}")),
        );
        write_dependency_manifest(
            &downstream_root,
            &downstream_name,
            &[&provider_coordinate, &coordinate],
        );
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![
                provider.artifact().path().to_path_buf(),
                consumer.artifact().path().to_path_buf(),
            ],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        runtime::check(
            &target,
            &[&core, &provider, consumer, &downstream],
            &runtime,
            &runtime_fixture,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
    }
}
