use super::*;

mod runtime;

#[test]
fn dependency_enum_values_compile_and_run_through_actual_artifacts() {
    let target =
        resolved_target().expect("the production test requires the configured LLVM target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-enums");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let provider_root = sysroot.path().join("provider");
    let coordinate = ConeCoordinate::new("dev.example", "enum-provider", "0.1.0").unwrap();
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "enum-provider",
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

    for case in [
        "standalone",
        "combined",
        "patterns",
        "constructors",
        "constructor-combinations",
    ] {
        let name = format!("enums-{case}");
        let root = sysroot.path().join(&name);
        write_manifest_cone(&root, "dev.example", &name, "library", &source(case));
        write_dependency_manifest(&root, &name, &[&coordinate]);
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
                vec![],
            );
            request.emit = StageDumpPolicy::Stage(kind);
            let output = request
                .build_and_publish()
                .unwrap_or_else(|error| panic!("{case} {stage}: {error:?}"));
            snapshot(
                &fixtures.join(format!("{case}.{stage}.snap")),
                output.emitted_dump().unwrap().text(),
            );
            outputs.push(output);
        }
        let output = outputs.last().unwrap();
        let downstream_name = format!("downstream-{case}");
        let downstream = sysroot.path().join(&downstream_name);
        let consumer_coordinate = ConeCoordinate::new("dev.example", &name, "0.1.0").unwrap();
        write_manifest_cone(
            &downstream,
            "dev.example",
            &downstream_name,
            "library",
            &source(if case.starts_with("constructor") {
                "constructor-downstream"
            } else if case == "patterns" {
                "pattern-downstream"
            } else {
                "downstream"
            }),
        );
        write_dependency_manifest(
            &downstream,
            &downstream_name,
            &[&coordinate, &consumer_coordinate],
        );
        let published_downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &downstream,
            &sysroot
                .path()
                .join(format!("output/{downstream_name}.slib")),
            vec![
                provider.artifact().path().to_path_buf(),
                output.artifact().path().to_path_buf(),
            ],
            vec![],
        )
        .build_and_publish()
        .unwrap_or_else(|error| panic!("downstream {case}: {error:?}"));
        runtime::check(
            &target,
            &[&core, &provider, output, &published_downstream],
            &fixtures,
            &sysroot.path().join(format!("run-{case}")),
        );
    }

    for case in [
        "wrong-identity",
        "wrong-payload",
        "c-abi",
        "non-exhaustive",
        "wrong-pattern-owner",
        "wrong-pattern-field",
        "wrong-pattern-style",
        "refutable-binding",
        "guarded-coverage",
        "unit-called",
        "named-positional",
        "positional-named",
        "constructor-missing-field",
        "constructor-duplicate-field",
        "constructor-wrong-payload",
        "constructor-wrong-owner",
        "constructor-shadowed",
        "constructor-type-arguments",
    ] {
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
            vec![provider.artifact().path().to_path_buf()],
            vec![],
        )
        .build_and_publish()
        .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("enum errors must be diagnosed by the HIR stage");
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("enum errors must precede MIR: {error:?}");
        };
        let text = diagnostics
            .iter()
            .map(|diagnostic| {
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
        snapshot(&fixtures.join(format!("{case}.diagnostic.snap")), &text);
        assert!(!destination.exists());
    }
}

fn snapshot(path: &Path, text: &str) {
    if std::env::var_os("SCOOP_UPDATE_IMPORTED_ENUMS").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
