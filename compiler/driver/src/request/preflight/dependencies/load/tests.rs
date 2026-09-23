use super::*;

#[test]
fn loader_retains_the_opened_bytes_when_the_locator_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.slib");
    std::fs::write(&path, b"initial artifact").unwrap();
    let loaded = LoadedExplicitDependencyInputs::load(
        &[HostArtifactLocator::new(&path).unwrap()],
        &[],
        DecodeLimits::default(),
    )
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
            DecodeLimits::default(),
        ),
        Err(ExplicitDependencyLoadError::NotRegularFile(input))
            if input.role() == ExplicitDependencyRole::Direct
                && input.index() == 0
                && input.path() == directory.path()
    ));
}

#[test]
fn loader_enforces_the_owned_byte_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("dependency.slib");
    std::fs::write(&path, b"too large").unwrap();
    let locator = HostArtifactLocator::new(&path).unwrap();
    let limits = DecodeLimits {
        owned_bytes: 3,
        ..DecodeLimits::default()
    };

    assert!(matches!(
        LoadedExplicitDependencyInputs::load(&[locator], &[], limits),
        Err(ExplicitDependencyLoadError::ArtifactTooLarge {
            input,
            actual: 9,
            limit: 3,
        }) if input.path() == path
    ));
}

#[test]
fn metered_loader_enforces_the_build_wide_snapshot_budget() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("dependency.slib");
    std::fs::write(&path, b"too large").unwrap();
    let locator = HostArtifactLocator::new(&path).unwrap();
    let mut values = scoop_slib::SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
    values.artifact_snapshot_bytes = 3;
    let limits = scoop_slib::SlibClosureDecodeLimitsV1::new(values).unwrap();
    let mut meter = SlibClosureDecodeMeterV1::new(limits);

    assert!(matches!(
        LoadedExplicitDependencyInputs::load_metered(
            &[locator],
            &[],
            DecodeLimits::default(),
            &mut meter,
        ),
        Err(ExplicitDependencyLoadError::Resource(
            scoop_slib::SlibClosureResourceErrorV1::LimitExceeded {
                resource: scoop_slib::SlibClosureResourceKindV1::ArtifactSnapshotBytes,
                limit: 3,
                observed: 9,
            }
        ))
    ));
}
