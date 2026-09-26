use super::*;

mod runtime;

#[test]
fn dependency_classes_compile_and_run_through_actual_artifacts() {
    let target =
        resolved_target().expect("the production test requires the configured LLVM target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-imported-classes");
    let source =
        |name: &str| std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap();
    let original_fingerprint = core.artifact().summary().artifact_fingerprint();
    let core_source = sysroot.path().join("editable-core");
    std::fs::rename(sysroot.path().join("lib/scoop.core"), &core_source).unwrap();
    std::fs::write(
        core_source.join("src/user_class.scoop"),
        source("core-provider"),
    )
    .unwrap();
    let changed = core_source.join("src/stage3_test.scoop");
    std::fs::write(
        &changed,
        std::fs::read_to_string(&changed)
            .unwrap()
            .replace("= 42", "= 43"),
    )
    .unwrap();
    let core = crate::normalize_direct_build_request(
        &core_source,
        vec![],
        vec![],
        core.artifact().path(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap()
    .build_and_publish()
    .unwrap();
    assert_ne!(
        original_fingerprint,
        core.artifact().summary().artifact_fingerprint()
    );
    std::fs::rename(core_source.join("src"), core_source.join("unused-source")).unwrap();
    let provider_root = sysroot.path().join("provider");
    let coordinate = ConeCoordinate::new("dev.example", "class-provider", "0.1.0").unwrap();
    write_manifest_cone(
        &provider_root,
        "dev.example",
        "class-provider",
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
    for case in [
        "exception-statement",
        "exception-expression",
        "exception-default",
        "standalone",
        "combined",
        "aliases",
        "secondary",
        "recursive",
        "core-extension",
        "virtual",
        "interface",
        "interface-alias",
        "interface-parent",
        "reference-identity",
        "core-interface",
        "interface-values",
        "interface-default-constructor",
    ] {
        let name = format!("classes-{case}");
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
        let name = format!("class-downstream-{case}");
        let root = sysroot.path().join(&name);
        let consumer =
            ConeCoordinate::new("dev.example", &format!("classes-{case}"), "0.1.0").unwrap();
        write_manifest_cone(
            &root,
            "dev.example",
            &name,
            "library",
            &source("downstream"),
        );
        write_dependency_manifest(&root, &name, &[&coordinate, &consumer]);
        let downstream = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &sysroot.path().join(format!("output/{name}.slib")),
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
            &[&core, &provider, output, &downstream],
            &runtime,
            &fixtures,
            &sysroot.path().join(format!("run-{case}")),
        );
    }
    for case in [
        "wrong-identity",
        "no-gc",
        "internal-constructor",
        "abstract-constructor",
        "interface-wrong-identity",
        "interface-wrong-argument",
        "throw-wrong-type",
        "throw-wrong-identity",
        "catch-wrong-type",
        "catch-expression-wrong-type",
        "catch-unreachable",
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
            panic!("class errors must be diagnosed by the HIR stage");
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("class errors must precede MIR: {error:?}");
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
    if std::env::var_os("SCOOP_UPDATE_IMPORTED_CLASSES").is_some() {
        std::fs::write(path, text).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        text,
        "{}",
        path.display()
    );
}
