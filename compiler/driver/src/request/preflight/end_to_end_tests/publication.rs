use super::*;

const WARNING_SOURCE: &str =
    include_str!("../../../../../../tests/fixtures/core-library/publication-warning.scoop");

#[test]
fn publication_failure_retains_current_warnings_for_core_and_ordinary_libraries() {
    let Some(target) = resolved_target() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(temp.path());
    let core_root = temp.path().join("lib/scoop.core");
    std::fs::write(
        core_root.join("src/publication-warning.scoop"),
        WARNING_SOURCE,
    )
    .unwrap();
    let core_output = temp.path().join("core.slib");
    let core_request = || {
        SingleConeBuildRequest::new(
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::cone_directory(&core_root),
            },
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
            TrustedCoreInput::BootstrapSelf,
            target.clone(),
            SlibOutputDestination::new(&core_output).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap()
    };
    assert_publication_warning(core_request(), &core_output, temp.path());
    std::fs::remove_dir(&core_output).unwrap();
    let core = core_request().build_and_publish().unwrap();
    assert_eq!(core.warnings().diagnostics().len(), 1);

    let ordinary_root = temp.path().join("ordinary");
    write_manifest_cone(
        &ordinary_root,
        "test",
        "publication",
        "library",
        WARNING_SOURCE,
    );
    let ordinary_output = temp.path().join("ordinary.slib");
    let ordinary = SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(&ordinary_root),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(&core_output).unwrap()),
        target,
        SlibOutputDestination::new(&ordinary_output).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    assert_publication_warning(ordinary, &ordinary_output, temp.path());
}

fn assert_publication_warning(request: SingleConeBuildRequest, output: &Path, temporary: &Path) {
    let loaded = request.load_preflight().unwrap();
    let validated = loaded.validate().unwrap();
    let parsed = validated.parse_current_sources().unwrap();
    let file = parsed
        .sources()
        .iter()
        .position(|source| source.text().text() == WARNING_SOURCE)
        .unwrap();
    std::fs::create_dir(output).unwrap();
    let error = parsed
        .build_and_publish(&temporary.join("staging"))
        .unwrap_err();
    assert!(
        matches!(error.cause(), CurrentConeProductionFailure::Publication(_)),
        "unexpected stage failure: {error:?}"
    );
    let warnings = error.warnings().unwrap();
    assert_eq!(warnings.diagnostics().len(), 1);
    let warning = &warnings.diagnostics()[0];
    let start = WARNING_SOURCE.find("Raedy ->").unwrap();
    assert_eq!(warning.file, file);
    assert_eq!(
        warning.span,
        Some(scoop_ast::Span::new(
            start.try_into().unwrap(),
            (start + "Raedy".len()).try_into().unwrap()
        ))
    );
    assert_eq!(warning.severity, scoop_ast::DiagnosticSeverity::Warning);
    assert_eq!(
        warning.message,
        "`Raedy` is a catch-all binding because `Signal` has no variant named `Raedy`; qualify an intended variant as `E.V`, or use `_` or an intentional binding name for a catch-all"
    );
    assert!(warnings.render_human().contains(":8:5: warning:"));
    assert!(output.is_dir());
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
}
