use super::*;

#[test]
fn loader_retains_the_opened_bytes_when_the_locator_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.slib");
    std::fs::write(&path, b"initial artifact").unwrap();
    let loaded =
        LoadedExplicitDependencyInputs::load(&[HostArtifactLocator::new(&path).unwrap()], &[])
            .unwrap();
    std::fs::write(&path, b"changed artifact").unwrap();
    assert_eq!(loaded.artifacts[0].bytes, b"initial artifact");
}

#[test]
fn loader_requires_a_regular_file() {
    let directory = tempfile::tempdir().unwrap();
    let locator = HostArtifactLocator::new(directory.path()).unwrap();

    assert!(matches!(
        LoadedExplicitDependencyInputs::load(
            &[locator],
            &[],

        ),
        Err(ExplicitDependencyLoadError::NotRegularFile(input))
            if input.role() == ExplicitDependencyRole::Direct
                && input.index() == 0
                && input.path() == directory.path()
    ));
}
