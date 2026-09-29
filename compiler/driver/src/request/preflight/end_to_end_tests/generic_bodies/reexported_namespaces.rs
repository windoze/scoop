use super::super::imported_classes::runtime;
use super::*;

#[test]
fn reexported_static_namespaces_republish_and_execute_from_artifacts() {
    let target = resolved_target().expect("static namespace imports require a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-reexported-namespaces");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let origin_coordinate =
        ConeCoordinate::new("dev.example", "namespace-origin", "0.1.0").unwrap();
    let origin_root = sysroot.path().join("origin");
    write_manifest_cone(
        &origin_root,
        "dev.example",
        origin_coordinate.name(),
        "library",
        &source("provider"),
    );
    let origin = build_manifest(
        sysroot.path(),
        &target,
        &origin_root,
        &sysroot.path().join("output/origin.slib"),
    );
    std::fs::rename(origin_root.join("src"), origin_root.join("unused-source")).unwrap();

    let mut facades = Vec::new();
    for name in ["namespace-first", "namespace-second"] {
        let coordinate = ConeCoordinate::new("dev.example", name, "0.1.0").unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source("facade"));
        write_dependency_manifest(&root, name, &[&origin_coordinate]);
        let facade = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
            vec![origin.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        facades.push((coordinate, facade));
    }
    let direct_coordinates = facades
        .iter()
        .map(|(coordinate, _)| coordinate)
        .collect::<Vec<_>>();
    let direct = facades
        .iter()
        .map(|(_, facade)| facade.artifact().path().to_path_buf())
        .collect::<Vec<_>>();
    let support = vec![origin.artifact().path().to_path_buf()];
    let runtime = runtime::build(&target, &sysroot.path().join("runtime"));
    let runtime_fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");

    for case in [
        "exact",
        "star",
        "deep",
        "objects",
        "companions",
        "enum-variants",
        "public-exact",
        "public-star",
    ] {
        eprintln!("reexported namespace case: {case}");
        let name = format!("namespace-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &direct_coordinates);
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
                direct.clone(),
                support.clone(),
            );
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            check_snapshot(
                &fixtures,
                case,
                stage,
                output.emitted_dump().unwrap().text(),
            );
            outputs.push(output);
        }
        let consumer = outputs.last().unwrap();
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        let downstream_name = format!("namespace-downstream-{case}");
        let downstream_root = sysroot.path().join(&downstream_name);
        let downstream_fixture = match case {
            "public-exact" => "exact-downstream",
            "public-star" => "star-downstream",
            _ => "downstream",
        };
        write_manifest_cone(
            &downstream_root,
            "dev.example",
            &downstream_name,
            "library",
            &source(downstream_fixture),
        );
        write_dependency_manifest(&downstream_root, &downstream_name, &[&coordinate]);
        let downstream_support = std::iter::once(origin.artifact().path().to_path_buf())
            .chain(direct.iter().cloned())
            .collect::<Vec<_>>();
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream_root,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![consumer.artifact().path().to_path_buf()],
            downstream_support.clone(),
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("{case} downstream: {error:?}"));
        runtime::check(
            &target,
            &[
                &core,
                &origin,
                &facades[0].1,
                &facades[1].1,
                consumer,
                &downstream,
            ],
            &runtime,
            &runtime_fixtures,
            &sysroot.path().join(format!("run-{case}")),
            &name,
        );

        if case == "public-star" {
            let case = "bad-reexported-hidden";
            let root = sysroot.path().join(case);
            let source = source(case);
            write_manifest_cone(&root, "dev.example", case, "library", &source);
            write_dependency_manifest(&root, case, &[&coordinate]);
            let destination = sysroot.path().join(format!("output/{case}.slib"));
            let error = build_manifest_request(
                sysroot.path(),
                &target,
                &root,
                &destination,
                vec![consumer.artifact().path().to_path_buf()],
                downstream_support,
            )
            .build_and_publish()
            .unwrap_err();
            check_rejection(&fixtures, case, &source, error);
            assert!(!destination.exists());
        }
    }

    let mut extras = Vec::new();
    for case in ["conflict-provider", "value-package-provider"] {
        let name = format!("namespace-{case}");
        let coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        let artifact = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
        );
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        extras.push((coordinate, artifact));
    }
    for (case, extra) in [
        ("bad-support-exact", None),
        ("bad-support-star", None),
        ("bad-private", None),
        ("bad-internal", None),
        ("bad-protected", None),
        ("bad-instance", None),
        ("bad-star-private", None),
        ("bad-public-private", None),
        ("bad-companion-private", None),
        ("bad-arity", None),
        ("bad-ambiguous-owner", Some(0)),
        ("bad-longest-package", Some(1)),
    ] {
        let root = sysroot.path().join(case);
        let source = source(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        let mut coordinates = direct_coordinates.clone();
        let mut artifacts = direct.clone();
        if let Some(index) = extra {
            coordinates.push(&extras[index].0);
            artifacts.push(extras[index].1.artifact().path().to_path_buf());
        }
        write_dependency_manifest(&root, case, &coordinates);
        let destination = sysroot.path().join(format!("output/{case}.slib"));
        let error = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &destination,
            artifacts,
            support.clone(),
        )
        .build_and_publish()
        .unwrap_err();
        check_rejection(&fixtures, case, &source, error);
        assert!(!destination.exists());
    }
}

fn check_rejection(fixtures: &Path, case: &str, source: &str, error: SingleConeProductionError) {
    let SingleConeProductionError::Production(error) = error else {
        panic!("{case} must fail during compilation: {error:?}");
    };
    let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
        diagnostics,
    )) = error.cause()
    else {
        panic!("{case} must fail in HIR: {error:?}");
    };
    assert!(!diagnostics.is_empty());
    let actual = diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.file, 0);
            let span = diagnostic.span.unwrap();
            format!(
                "span={}..{}\nexpression={}\n{}\n",
                span.start,
                span.end,
                &source[span.start as usize..span.end as usize],
                diagnostic.message
            )
        })
        .collect::<String>();
    check_snapshot(fixtures, case, "diagnostic", &actual);
}

fn check_snapshot(fixtures: &Path, case: &str, stage: &str, actual: &str) {
    let path = fixtures.join(format!("{case}.{stage}.snap"));
    if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(
        actual,
        std::fs::read_to_string(path).unwrap(),
        "{case} {stage}"
    );
}
