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
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::cone_directory(&root),
            },
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
