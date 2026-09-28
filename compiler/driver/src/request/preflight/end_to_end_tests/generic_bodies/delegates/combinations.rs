use super::*;

#[test]
fn generic_delegate_values_and_order_republish_and_execute() {
    check_cases(&[
        "order",
        "write-only",
        "read-only-var",
        "zst",
        "flat",
        "references",
        "combined",
    ]);
}

#[test]
fn generic_delegate_aliases_cycles_and_local_source_republish_and_execute() {
    check_cases(&[
        "aliases",
        "cycle-direct",
        "cycle-indirect",
        "local-source",
        "foreign-receiver",
    ]);
}

fn check_cases(cases: &[&str]) {
    check_fixture_cases("m23-generic-delegate-combinations", cases);
}

fn check_fixture_cases(fixture: &str, cases: &[&str]) {
    let target = resolved_target().expect("generic delegate combinations require a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures").join(fixture);
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "delegate-combinations-provider", "0.1.0").unwrap();
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
    let runtime_fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    for &case in cases {
        eprintln!("generic delegate case: {case}");
        let name = format!("delegate-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &[&provider_coordinate]);
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
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            let actual = output.emitted_dump().unwrap().text();
            if stage == "mir" && case == "read-only-var" {
                assert!(
                    !actual.contains("  fun setValue "),
                    "a getter does not materialize the setter role"
                );
            }
            if stage == "mir" && case == "write-only" {
                assert!(
                    !actual.contains("  fun getValue "),
                    "a setter does not materialize the getter role"
                );
            }
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(
                actual,
                std::fs::read_to_string(snapshot).unwrap(),
                "{case} {stage}"
            );
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("delegate-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source(match case {
                "aliases" => "aliases-downstream",
                "foreign-local" => "foreign-local-downstream",
                _ => "downstream",
            }),
        );
        write_dependency_manifest(&downstream_root, &downstream_name, &[&coordinate]);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![consumer.artifact().path().to_path_buf()],
            vec![provider.artifact().path().to_path_buf()],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        let closure = runtime::check(
            &target,
            &[&core, &provider, consumer, &downstream],
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        let (source, _) = closure
            .artifact(provider_coordinate.identity().unwrap())
            .unwrap();
        assert!(
            source
                .lir_strong_production()
                .initialization_registrations()
                .registrations()
                .iter()
                .all(|unit| unit.definition_owner()
                    == scoop_lir::RegistrationDefinitionOwner::Strong),
            "the provider has not instantiated a generic delegate"
        );
        let (sections, _) = closure.artifact(coordinate.identity().unwrap()).unwrap();
        let production = sections.lir_strong_production().registration_production();
        let units = production.initialization_units().registrations();
        if case == "foreign-local" {
            assert!(units.is_empty(), "local delegates use lexical storage");
            continue;
        }
        assert!(!units.is_empty(), "{case} has concrete delegate units");
        for unit in units {
            if matches!(
                case,
                "initializer-local"
                    | "initializer-closure"
                    | "initializer-combined"
                    | "initializer-anonymous"
                    | "initializer-values"
                    | "initializer-local-closure"
            ) {
                let dependencies = unit.semantic().dependencies();
                assert_eq!(dependencies.len(), 1, "Trace is one external unit");
                assert!(matches!(dependencies[0].kind(),
                    scoop_lir::StrongInitializationDependencyKindV2::DependencyExternalUnit {
                        provider,
                        ..
                    } if provider == provider_coordinate.identity().unwrap()));
            }
            assert_eq!(
                unit.semantic().schedule(),
                scoop_lir::StrongInitializationSchedulePlanV1::LazyAccess
            );
            assert!(matches!(
                unit.definition_owner(),
                scoop_lir::RegistrationDefinitionOwner::Odr { .. }
            ));
            let storage = production
                .static_storages()
                .registrations()
                .iter()
                .find(|storage| storage.semantic().storage() == unit.storage().storage())
                .unwrap();
            if case == "zst" {
                assert_eq!(storage.semantic().byte_size(), 0);
                assert_eq!(storage.semantic().allocation_extent(), 1);
                assert_eq!(
                    storage.semantic().scan_kind(),
                    scoop_lir::StaticStorageScanKindV1::None
                );
            }
            if matches!(case, "flat" | "references") {
                assert_eq!(storage.semantic().byte_size(), 24);
                assert_eq!(
                    storage.semantic().scan_kind(),
                    if case == "flat" {
                        scoop_lir::StaticStorageScanKindV1::None
                    } else {
                        scoop_lir::StaticStorageScanKindV1::Recursive
                    }
                );
            }
        }
    }
}

#[test]
fn generic_delegates_use_dependency_members_and_local_accessors() {
    check_cases(&["foreign-delegate", "foreign-local"]);
}

#[test]
fn generic_delegate_mixed_roles_republish_and_execute() {
    check_cases(&["mixed-members", "foreign-extensions"]);
}

#[test]
fn generic_delegate_initializer_local_functions_republish_and_execute() {
    check_fixture_cases(
        "m23-generic-delegate-generated",
        &["initializer-basic", "initializer-local"],
    );
}

#[test]
fn generic_delegate_initializer_closures_republish_and_execute() {
    check_fixture_cases(
        "m23-generic-delegate-closures",
        &[
            "initializer-closure",
            "initializer-combined",
            "initializer-plain",
            "initializer-anonymous",
            "initializer-values",
            "initializer-local-closure",
        ],
    );
}
