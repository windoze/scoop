use super::super::imported_classes::runtime;
use super::*;

mod dispatch;

#[test]
fn generic_initializations_survive_publication_and_execute_with_moving_gc() {
    check_initializations(&["standalone", "combined"]);
}

#[test]
fn generic_dispatch_survives_publication_and_executes_with_moving_gc() {
    check_initializations(&["dispatch", "dispatch-combined"]);
}

fn check_initializations(cases: &[&str]) {
    let target = resolved_target().expect("generic initialization requires a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-initialization");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let runtime_fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");

    for &case in cases {
        let provider_name = format!("generic-initialization-{case}");
        let provider_coordinate =
            ConeCoordinate::new("dev.example", &provider_name, "0.1.0").unwrap();
        let provider_root = sysroot.path().join(&provider_name);
        write_manifest_cone(
            &provider_root,
            "dev.example",
            &provider_name,
            "library",
            &source(case),
        );
        let mut outputs = Vec::new();
        for (kind, stage) in [(StageDumpKind::Mir, "mir"), (StageDumpKind::Lir, "lir")] {
            let mut request = build_manifest_request(
                sysroot.path(),
                &target,
                &provider_root,
                &sysroot.path().join(format!("output/{provider_name}.slib")),
                Vec::new(),
                Vec::new(),
            );
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            let actual = output.emitted_dump().unwrap().text();
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let provider = outputs.last().unwrap();
        std::fs::rename(
            provider_root.join("src"),
            provider_root.join("unused-source"),
        )
        .unwrap();

        let consumer_name = format!("generic-initialization-consumer-{case}");
        let consumer_coordinate =
            ConeCoordinate::new("dev.example", &consumer_name, "0.1.0").unwrap();
        let consumer_root = sysroot.path().join(&consumer_name);
        write_manifest_cone(
            &consumer_root,
            "dev.example",
            &consumer_name,
            "library",
            &source("consumer"),
        );
        write_dependency_manifest(&consumer_root, &consumer_name, &[&provider_coordinate]);
        let consumer = build_manifest_request(
            sysroot.path(),
            &target,
            &consumer_root,
            &sysroot.path().join(format!("output/{consumer_name}.slib")),
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} consumer: {error:?}"));
        std::fs::rename(
            consumer_root.join("src"),
            consumer_root.join("unused-source"),
        )
        .unwrap();

        let downstream_name = format!("generic-initialization-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source("downstream"),
        );
        write_dependency_manifest(&downstream_root, &downstream_name, &[&consumer_coordinate]);
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
            &[&core, provider, &consumer, &downstream],
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            case,
        );
        let (sections, _) = closure
            .artifact(provider_coordinate.identity().unwrap())
            .unwrap();
        assert_eq!(
            sections
                .hir_interface()
                .generic_initializations()
                .records()
                .len(),
            if case == "combined" { 5 } else { 2 },
        );
        let production = sections.lir_strong_production();
        let shapes = production.canonical_shape_definitions().definitions();
        let expected_types = if case == "standalone" { 2 } else { 4 };
        if case.starts_with("dispatch") {
            dispatch::check(sections, if case == "dispatch" { 1 } else { 2 });
            assert!(!shapes.is_empty());
        } else {
            assert_eq!(shapes.len(), expected_types * 6);
        }
        for shape in shapes {
            let definition = closure
                .odr_definitions()
                .get(shape.group(), shape.member())
                .unwrap();
            assert_eq!(definition.key().role(), shape.role());
            assert_eq!(definition.abi().as_array(), shape.abi().as_array());
            assert!(definition.candidates().any(|candidate| {
                candidate.provider() == provider_coordinate.identity().unwrap()
            }));
        }
        let mut registrations = 0;
        for plan in production.type_registrations().registrations() {
            if let scoop_lir::RegistrationDefinitionOwner::Odr { group, member } =
                plan.definition_owner()
            {
                registrations += 1;
                let definition = closure.odr_definitions().get(group, member).unwrap();
                assert_eq!(
                    definition.key().role(),
                    scoop_identity::OdrMemberRole::RegistrationRecord,
                );
            }
        }
        if case.starts_with("dispatch") {
            assert!(registrations >= 2);
        } else {
            assert_eq!(registrations, expected_types);
        }
    }
}
