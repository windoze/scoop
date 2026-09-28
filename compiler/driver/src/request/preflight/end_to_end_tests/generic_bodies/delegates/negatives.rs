use super::*;

#[test]
fn generic_delegate_language_errors_have_source_diagnostics() {
    let target = resolved_target().expect("generic delegate diagnostics require a target");
    let sysroot = tempfile::tempdir().unwrap();
    let _core = bootstrap_core(sysroot.path(), &target);
    let fixtures = crate::workspace_root().join("tests/fixtures/m23-generic-delegate-combinations");
    let provider_coordinate =
        ConeCoordinate::new("dev.example", "delegate-combinations-provider", "0.1.0").unwrap();
    let provider_root = sysroot.path().join("provider");
    write_manifest_cone(
        &provider_root,
        "dev.example",
        provider_coordinate.name(),
        "library",
        &std::fs::read_to_string(fixtures.join("provider.scoop")).unwrap(),
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
        "bad-result-inference",
        "bad-rhs-inference",
        "bad-this-in-by",
        "bad-suspend-role",
        "bad-generic-role",
        "bad-default-role",
        "bad-vararg-role",
        "bad-provide-receiver",
        "bad-set-result",
        "bad-missing-operator",
        "bad-missing-setter",
        "bad-suspend-initializer",
        "bad-readonly",
        "bad-result-type",
        "bad-rhs-type",
        "bad-runtime-receiver-inference",
        "bad-mixed-member-ambiguity",
        "extension-property-rules",
    ] {
        let source_path = if case == "extension-property-rules" {
            crate::workspace_root().join(
                "tests/fixtures/m21-extension-properties/errors/extension-property-rules.scoop",
            )
        } else {
            fixtures.join(format!("{case}.scoop"))
        };
        let source = std::fs::read_to_string(&source_path).unwrap();
        let root = sysroot.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "library", &source);
        write_dependency_manifest(&root, case, &[&provider_coordinate]);
        let destination = sysroot.path().join(format!("output/{case}.slib"));
        let error = build_manifest_request(
            sysroot.path(),
            &target,
            &root,
            &destination,
            vec![provider.artifact().path().to_path_buf()],
            Vec::new(),
        )
        .build_and_publish()
        .unwrap_err();
        let SingleConeProductionError::Production(error) = error else {
            panic!("{case} must be diagnosed during compilation: {error:?}")
        };
        let CurrentConeProductionFailure::Hir(current_hir::CurrentConeHirStageError::Lowering(
            diagnostics,
        )) = error.cause()
        else {
            panic!("{case} must be diagnosed before MIR: {error:?}")
        };
        assert!(!diagnostics.is_empty(), "{case}");
        let actual = diagnostics
            .iter()
            .map(|diagnostic| {
                assert_eq!(
                    diagnostic.file, 0,
                    "{case}: the error belongs to the consumer"
                );
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
        let snapshot = if case == "extension-property-rules" {
            source_path.with_extension("scoop.snap")
        } else {
            fixtures.join(format!("{case}.diagnostic.snap"))
        };
        if std::env::var_os("SCOOP_UPDATE_GENERIC_BODY_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap(), "{case}");
        assert!(!destination.exists(), "{case} cannot publish an artifact");
    }
}
