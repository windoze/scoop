use super::super::imported_classes::runtime;
use super::*;

mod conflicts;
mod definitions;
mod unions;

#[test]
fn sibling_adapters_union_independent_members_and_share_runtime_types() {
    check_siblings("m23-odr-member-unions", &["adapters", "adapters-combined"]);
}

fn check_siblings(fixture_name: &str, cases: &[&str]) {
    let target = resolved_target().expect("sibling generic publication requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root()
        .join("tests/fixtures")
        .join(fixture_name);
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "generic-odr-provider", "0.1.0").unwrap();
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

    for &case in cases {
        let snapshots = case.starts_with("strings") || case.starts_with("adapters");
        let mut siblings = Vec::new();
        let mut coordinates = Vec::new();
        for side in ["left", "right"] {
            let name = format!("generic-odr-{case}-{side}");
            let root = sysroot.path().join(&name);
            write_manifest_cone(
                &root,
                "dev.example",
                &name,
                "library",
                &source(&format!("{case}-{side}")),
            );
            write_dependency_manifest(&root, &name, &[&provider_coordinate]);
            let request = || {
                build_manifest_request(
                    sysroot.path(),
                    &target,
                    &root,
                    &sysroot.path().join(format!("output/{name}.slib")),
                    vec![provider.artifact().path().to_path_buf()],
                    Vec::new(),
                )
            };
            if snapshots {
                for (kind, stage) in [(StageDumpKind::Hir, "hir"), (StageDumpKind::Mir, "mir")] {
                    let mut request = request();
                    request.emit = StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(kind));
                    let output = request
                        .build_and_publish()
                        .unwrap_or_else(|error| panic!("{case} {side} {stage}: {error:?}"));
                    check_snapshot(
                        &fixtures,
                        case,
                        side,
                        stage,
                        output.emitted_dumps().first().unwrap().text(),
                    );
                }
            }
            let mut request = request();
            if snapshots {
                request.emit =
                    StageDumpPolicy::Stages(scoop_protocol::StageDumpSet::one(StageDumpKind::Lir));
            }
            let artifact = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {side}: {error:?}"));
            if snapshots {
                check_snapshot(
                    &fixtures,
                    case,
                    side,
                    "lir",
                    artifact.emitted_dumps().first().unwrap().text(),
                );
            }
            std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
            coordinates.push(ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap());
            siblings.push(artifact);
        }
        let name = format!("generic-odr-{case}-consumer");
        let root = sysroot.path().join(&name);
        write_manifest_cone(
            &root,
            "dev.example",
            &name,
            "library",
            &source(&format!("{case}-consumer")),
        );
        write_dependency_manifest(
            &root,
            &name,
            &[&provider_coordinate, &coordinates[0], &coordinates[1]],
        );
        let consumer = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![
                provider.artifact().path().to_path_buf(),
                siblings[0].artifact().path().to_path_buf(),
                siblings[1].artifact().path().to_path_buf(),
            ],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} consumer: {error:?}"));
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let left = coordinates[0].identity().unwrap();
        let right = coordinates[1].identity().unwrap();
        let closure = runtime::read(
            &target,
            &[&core, &provider, &siblings[0], &siblings[1], &consumer],
        );
        let mut template = std::fs::read_to_string(runtime_fixture.join("runtime.c")).unwrap();
        if case.starts_with("adapters") {
            unions::check_members(&closure, left, right);
            template = unions::runtime_source(
                &closure,
                provider_coordinate.identity().unwrap(),
                left,
                right,
                &template,
                &std::fs::read_to_string(fixtures.join("addresses.c")).unwrap(),
            );
        }
        runtime::execute(
            &target,
            &[&core, &provider, &siblings[0], &siblings[1], &consumer],
            &closure,
            &runtime,
            &template,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        definitions::check(&closure, left, right, case);
    }
}

fn check_snapshot(fixtures: &std::path::Path, case: &str, side: &str, stage: &str, actual: &str) {
    let path = fixtures.join(format!("{case}-{side}.{stage}.snap"));
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
