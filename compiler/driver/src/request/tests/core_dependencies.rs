use super::*;

#[test]
fn core_request_retains_direct_and_support_inputs_for_common_preflight() {
    let directory = TempDirectory::new();
    std::fs::write(
        directory.0.join("Cone.toml"),
        include_str!(
            "../../../../../tests/fixtures/language/core-library/dependencies/core/Cone.toml"
        ),
    )
    .unwrap();
    let direct = directory.0.join("helper.slib");
    let support = directory.0.join("support.slib");
    std::fs::write(&direct, b"malformed helper artifact").unwrap();
    std::fs::write(&support, b"malformed support artifact").unwrap();
    let request = SingleConeBuildRequest::new(
        crate::request::test_support::expanded_fixture(ManifestRootLocator::cone_directory(
            &directory.0,
        )),
        ExplicitDependencyInputs::new(
            vec![HostArtifactLocator::new(&direct).unwrap()],
            vec![HostArtifactLocator::new(&support).unwrap()],
        ),
        TrustedCoreInput::BootstrapSelf,
        ResolvedTargetProfile::resolve_host().unwrap(),
        SlibOutputDestination::new(directory.0.join("core.slib")).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    assert!(matches!(
        request.trusted_core,
        TrustedCoreInput::BootstrapSelf
    ));
    assert_eq!(request.dependencies.direct()[0].as_path(), direct);
    assert_eq!(request.dependencies.support()[0].as_path(), support);
    let loaded = request.load_preflight().unwrap();
    assert!(matches!(
        loaded.validate(),
        Err(preflight::SingleConeDependencyValidationError::ExplicitDependencies(error))
            if matches!(error.as_ref(), preflight::ExplicitDependencyValidationError::Summary { input, .. }
                if input.role() == preflight::ExplicitDependencyRole::Direct && input.path() == direct)
    ));
    assert!(!directory.0.join("src").exists());
}
