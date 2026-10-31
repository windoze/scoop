use super::*;

const BINDING_SOURCE: &str = include_str!(
    "../../../../../../tests/fixtures/language/core-library/publication-bindings.scoop"
);

#[test]
fn publication_failures_preserve_destinations_and_current_warnings() {
    let target = resolved_target().expect("publication tests require a supported target");
    let temp = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(temp.path());
    let core_root = temp.path().join("src/scoop.core");
    std::fs::write(
        core_root.join("src/publication-bindings.scoop"),
        BINDING_SOURCE,
    )
    .unwrap();
    let core_output = temp.path().join("core.slib");
    let core_request = || {
        SingleConeBuildRequest::new(
            crate::request::test_support::expanded_fixture(ManifestRootLocator::cone_directory(
                &core_root,
            )),
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
            TrustedCoreInput::BootstrapSelf,
            target.clone(),
            SlibOutputDestination::new(&core_output).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap()
    };
    let failure = publication_failure(core_request(), &core_output, temp.path());
    assert!(failure.warnings().unwrap().diagnostics().is_empty());
    std::fs::remove_dir(&core_output).unwrap();
    let core = core_request().build_and_publish().unwrap();
    assert!(core.warnings().diagnostics().is_empty());

    let ordinary_root = temp.path().join("ordinary");
    write_manifest_cone(
        &ordinary_root,
        "test",
        "publication",
        "library",
        BINDING_SOURCE,
    );
    let ordinary_output = temp.path().join("ordinary.slib");
    let ordinary = SingleConeBuildRequest::new(
        crate::request::test_support::expanded_fixture(ManifestRootLocator::cone_directory(
            &ordinary_root,
        )),
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(&core_output).unwrap()),
        target.clone(),
        SlibOutputDestination::new(&ordinary_output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let failure = publication_failure(ordinary, &ordinary_output, temp.path());
    assert!(failure.warnings().unwrap().diagnostics().is_empty());

    let source = temp.path().join("single-file.scoop");
    std::fs::write(&source, "package test\nfun main() {}\n").unwrap();
    let standard =
        super::super::single_file_tests::standard_library(&target, &core_output, temp.path());
    let single_output = temp.path().join("single-file.slib");
    let single = SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: scoop_manifest::SingleFileLocator::from_path(&source).unwrap(),
        },
        ExplicitDependencyInputs::new(
            vec![HostArtifactLocator::new(standard).unwrap()],
            Vec::new(),
        ),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(&core_output).unwrap()),
        target,
        SlibOutputDestination::new(&single_output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let failure = publication_failure(single, &single_output, temp.path());
    let warnings = failure.warnings().unwrap();
    assert_eq!(warnings.diagnostics().len(), 1);
    let warning = &warnings.diagnostics()[0];
    assert_eq!(warning.file, 0);
    assert_eq!(warning.span, Some(scoop_ast::Span::new(0, 12)));
    assert_eq!(warning.severity, scoop_ast::DiagnosticSeverity::Warning);
    assert_eq!(
        warning.message,
        "package declaration is ignored in single-file mode; using the default root package"
    );
    let rendered = warnings.render_human();
    assert!(rendered.starts_with("warning:"), "{rendered}");
    assert!(rendered.contains(":1:1"), "{rendered}");
    assert!(rendered.contains("1 | package test"), "{rendered}");
}

fn publication_failure(
    request: SingleConeBuildRequest,
    output: &Path,
    temporary: &Path,
) -> CurrentConeProductionError {
    let loaded = request.load_preflight().unwrap();
    let validated = loaded.validate().unwrap();
    let parsed = validated.parse_current_sources().unwrap();
    std::fs::create_dir(output).unwrap();
    let error = parsed
        .build_and_publish(&temporary.join("staging"))
        .unwrap_err();
    assert!(
        matches!(error.cause(), CurrentConeProductionFailure::Publication(_)),
        "unexpected stage failure: {error:?}"
    );
    assert!(output.is_dir());
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
    error
}
