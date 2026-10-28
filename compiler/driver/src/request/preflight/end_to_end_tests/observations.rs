use super::*;

#[test]
fn observations_stop_at_requested_stage_without_publishing_artifacts() {
    let target = resolved_target().expect("observation tests require a host target");
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    let root = sysroot.path().join("observed");
    write_manifest_cone(
        &root,
        "test",
        "observed",
        "library",
        "fun answer(): Long = 42\n",
    );
    let output = sysroot.path().join("observed.slib");
    for stage in StageDumpKind::ALL {
        let request = SingleConeBuildRequest::new(
            crate::request::test_support::expanded_fixture(ManifestRootLocator::cone_directory(
                &root,
            )),
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
            TrustedCoreInput::Artifact(HostArtifactLocator::new(core.artifact().path()).unwrap()),
            target.clone(),
            SlibOutputDestination::new(&output).unwrap(),
            DiagnosticOutputPolicy::Structured,
            StageDumpPolicy::Stages(super::super::super::StageDumpSet::one(stage)),
        )
        .unwrap();
        let result = request.compile(true).unwrap();
        assert!(matches!(result, SingleConeCompilation::Observed(_)));
        assert_eq!(result.emitted_dumps().len(), 1);
        assert_eq!(result.emitted_dumps()[0].kind(), stage);
        if stage == StageDumpKind::Optimizations {
            let report: serde_json::Value =
                serde_json::from_str(result.emitted_dumps()[0].text()).unwrap();
            assert_eq!(report["schema"], 2);
            assert_eq!(report["rounds"], 0);
            assert_eq!(report["converged"], true);
            assert_eq!(report["passes"]["inline"], false);
            assert_eq!(report["passes"]["allocation-elision"], false);
            assert!(report["events"].as_array().unwrap().is_empty());
            let answer = report["functions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|function| function["name"] == "answer")
                .unwrap();
            assert_eq!(answer["declaration"]["source"]["path"], "src/main.scoop");
            assert_eq!(answer["materialization"]["template"]["kind"], "function");
        }
        if stage == StageDumpKind::Symbols {
            let report: serde_json::Value =
                serde_json::from_str(result.emitted_dumps()[0].text()).unwrap();
            assert_eq!(report["schema"], 1);
            let symbols = report["symbols"].as_array().unwrap();
            let answer = symbols
                .iter()
                .find(|entry| entry["name"] == "answer")
                .unwrap();
            assert_eq!(answer["producer"], report["cone"]);
            assert_eq!(answer["template"]["kind"], "function");
            assert_eq!(answer["implementation"]["kind"], "strong");
            assert_eq!(answer["declaration"]["source"]["path"], "src/main.scoop");
            assert_eq!(answer["signature"]["parameters"], serde_json::json!([]));
            assert_eq!(
                answer["symbol"].as_str().unwrap(),
                format!("scoop$1$cb${}", answer["body"].as_str().unwrap())
            );
            assert!(
                symbols
                    .iter()
                    .any(|entry| entry["producer"] != report["cone"])
            );
        }
        assert!(!output.exists());
    }
    // Parsing an AST observation must not run the type checker.
    write_manifest_cone(
        &root,
        "test",
        "observed",
        "library",
        "fun answer(): Long = undefinedName\n",
    );
    let mut request = build_manifest_request(
        sysroot.path(),
        &target,
        &root,
        &output,
        Vec::new(),
        Vec::new(),
    );
    request.emit =
        StageDumpPolicy::Stages(super::super::super::StageDumpSet::one(StageDumpKind::Ast));
    assert!(matches!(
        request.compile(true).unwrap(),
        SingleConeCompilation::Observed(_)
    ));
    assert!(!output.exists());
}
