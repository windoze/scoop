use super::*;

#[test]
fn missing_dependency_keeps_input_location_and_role() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing.slib");
    let error = LoadedExplicitDependencyInputs::load(
        &[HostArtifactLocator::new(&path).unwrap()],
        &[],
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap_err();
    assert!(
        matches!(error, ExplicitDependencyLoadError::Io { input, source, .. }
        if source.kind() == std::io::ErrorKind::NotFound
            && input.role() == ExplicitDependencyRole::Direct
            && input.index() == 0 && input.path() == path)
    );
}
