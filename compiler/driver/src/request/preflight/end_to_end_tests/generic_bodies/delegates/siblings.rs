use super::*;

#[test]
fn generic_delegate_siblings_share_initialization_and_failure_with_moving_gc() {
    let target = resolved_target().expect("generic delegate siblings require a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-delegate-siblings");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "delegate-siblings-provider", "0.1.0").unwrap();
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
    let mut siblings = Vec::new();
    let mut coordinates = Vec::new();
    for side in ["left", "right"] {
        let name = format!("delegate-siblings-{side}");
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(side));
        write_dependency_manifest(&root, &name, &[&provider_coordinate]);
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
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{side} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{side}.{stage}.snap"));
            let actual = output.emitted_dump().unwrap().text();
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            if stage == "lir" {
                siblings.push(output);
            }
        }
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        coordinates.push(ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap());
    }
    let name = "delegate-siblings-consumer";
    let root = sysroot.path().join(name);
    write_manifest_cone(&root, "dev.example", name, "library", &source("consumer"));
    write_dependency_manifest(
        &root,
        name,
        &[&provider_coordinate, &coordinates[0], &coordinates[1]],
    );
    let consumer = build_manifest_request(
        sysroot.path(),
        &target,
        &root,
        &sysroot.path().join("output/consumer.slib"),
        vec![
            provider.artifact().path().to_path_buf(),
            siblings[0].artifact().path().to_path_buf(),
            siblings[1].artifact().path().to_path_buf(),
        ],
        Vec::new(),
    )
    .build_and_publish()
    .unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let closure = runtime::check(
        &target,
        &[&core, &provider, &siblings[0], &siblings[1], &consumer],
        &runtime,
        &crate::workspace_root().join("tests/fixtures/m23-imported-classes"),
        &sysroot.path().join("run"),
        "generic-delegate-siblings",
    );
    let units = coordinates
        .iter()
        .map(|coordinate| {
            let (sections, _) = closure.artifact(coordinate.identity().unwrap()).unwrap();
            sections
                .lir_strong_production()
                .registration_production()
                .initialization_units()
                .registrations()
        })
        .collect::<Vec<_>>();
    assert_eq!(units[0].len(), 2);
    assert_eq!(units[1].len(), 5);
    for first in units[0] {
        let second = units[1]
            .iter()
            .find(|candidate| candidate.semantic().unit() == first.semantic().unit())
            .unwrap();
        assert_eq!(first.cell_symbol(), second.cell_symbol());
        assert_eq!(first.storage().storage(), second.storage().storage());
        assert_eq!(
            first.failure_root().storage(),
            second.failure_root().storage()
        );
        let scoop_lir::RegistrationDefinitionOwner::Odr { group, member } =
            first.definition_owner()
        else {
            panic!("shared delegate units have ODR registration identities")
        };
        let merged = closure.odr_definitions().get(group, member).unwrap();
        assert_eq!(merged.candidates().count(), 2);
    }
}
