use super::members::check_fixture_cases;
use super::*;

#[test]
fn qualified_dependency_types_republish_and_execute_from_artifacts() {
    check_fixture_cases(
        "m23-qualified-types",
        &["ordinary", "aliases", "nested", "bounds"],
        &[
            "bad-alias-arguments",
            "bad-arity",
            "bad-missing-arguments",
            "bad-nongeneric",
            "bad-hidden",
            "bad-package",
            "bad-current-longer",
            "bad-conflict",
            "bad-nested-arity",
            "bad-nested-missing",
            "bad-nested-private",
            "bad-nested-bound",
            "bad-namespace-alias-cycle",
        ],
        "downstream",
    );
}

#[test]
fn qualified_dependency_types_merge_current_package_contributions() {
    check_fixture_cases(
        "m23-qualified-types",
        &["split-package"],
        &[],
        "split-downstream",
    );
}

#[test]
fn qualified_dependency_types_respect_direct_package_visibility() {
    let target = resolved_target().expect("qualified dependency types require a target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-qualified-types");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let origin_coordinate =
        ConeCoordinate::new("dev.example", "qualified-origin", "0.1.0").unwrap();
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
    for name in ["qualified-first", "qualified-second"] {
        let coordinate = ConeCoordinate::new("dev.example", name, "0.1.0").unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(
            &root,
            "dev.example",
            name,
            "library",
            &source("reexport-provider"),
        );
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
    let mut extras = Vec::new();
    for name in ["conflict-provider", "value-package-provider"] {
        let coordinate = ConeCoordinate::new("dev.example", name, "0.1.0").unwrap();
        let root = sysroot.path().join(name);
        write_manifest_cone(&root, "dev.example", name, "library", &source(name));
        let output = build_manifest(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
        );
        std::fs::rename(root.join("src"), root.join("unused-source")).unwrap();
        extras.push((coordinate, output));
    }

    let direct_coordinates = facades
        .iter()
        .map(|(coordinate, _)| coordinate)
        .collect::<Vec<_>>();
    let direct = facades
        .iter()
        .map(|(_, artifact)| artifact.artifact().path().to_path_buf())
        .collect::<Vec<_>>();
    let support = vec![origin.artifact().path().to_path_buf()];
    for (case, extra) in [("diamond", false), ("independent", true)] {
        let name = format!("qualified-{case}");
        let mut coordinates = direct_coordinates.clone();
        let mut artifacts = direct.clone();
        if extra {
            coordinates.push(&extras[1].0);
            artifacts.push(extras[1].1.artifact().path().to_path_buf());
        }
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &coordinates);
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
                &sysroot.path().join(format!("output/{case}.slib")),
                artifacts.clone(),
                support.clone(),
            );
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request.build_and_publish().unwrap();
            let actual = output.emitted_dump().unwrap().text();
            let snapshot = fixtures.join(format!("{case}.{stage}.snap"));
            if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            outputs.push(output);
        }
        let runtime = super::super::imported_classes::runtime::build(
            &target,
            &sysroot.path().join("runtime"),
        );
        let mut libraries = vec![&core, &origin, &facades[0].1, &facades[1].1];
        if extra {
            libraries.push(&extras[1].1);
        }
        libraries.push(outputs.last().unwrap());
        super::super::imported_classes::runtime::check(
            &target,
            &libraries,
            &runtime,
            &crate::workspace_root().join("tests/fixtures/m23-imported-classes"),
            &sysroot.path().join(format!("run-{case}")),
            &name,
        );
    }

    for (case, extra, expected) in [
        ("bad-support-package", None, "unknown type `dependency`"),
        ("bad-direct-conflict", Some(0), "ambiguous"),
        (
            "bad-direct-longest",
            Some(1),
            "package `dependency.paths.Outer` has no accessible type `Nested`",
        ),
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
        let SingleConeProductionError::Production(error) = error else {
            panic!("{case} must fail during compilation: {error:?}");
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("{case} must fail in HIR: {error:?}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{case}: {diagnostics:?}"
        );
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
        let snapshot = fixtures.join(format!("{case}.diagnostic.snap"));
        if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
        assert!(!destination.exists());
    }
}
