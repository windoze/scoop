use super::*;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::ConeRecord;

const TARGET: ValidatedLirTargetSelection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;

fn manifest_artifact(coordinate: ConeCoordinate, producer: &str) -> Vec<u8> {
    let cone = ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap();
    crate::test_artifacts::manifest_archive(
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        cone,
        producer,
        Vec::new(),
    )
    .into_bytes()
}

#[test]
fn explicit_artifact_is_probed_without_becoming_a_compile_or_link_proof() {
    let temp = tempfile::tempdir().unwrap();
    let artifact = temp.path().join("dep.slib");
    let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
    let bytes = manifest_artifact(coordinate.clone(), "locator-test");
    std::fs::write(&artifact, &bytes).unwrap();

    let prebuilt = locate_artifact_dependency(
        artifact.clone(),
        &coordinate,
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    assert_eq!(prebuilt.coordinate(), &coordinate);
    assert_eq!(prebuilt.candidate_count(), 1);
    assert_eq!(
        prebuilt.artifact_fingerprint(),
        prebuilt.first_candidate().summary().artifact_fingerprint()
    );

    std::fs::write(
        &artifact,
        manifest_artifact(
            ConeCoordinate::new("test", "other", "1.0.0").unwrap(),
            "wrong-coordinate",
        ),
    )
    .unwrap();
    assert!(matches!(
        locate_artifact_dependency(
            artifact.clone(),
            &coordinate,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::CoordinateMismatch { .. })
    ));

    std::fs::write(&artifact, &bytes).unwrap();
}

#[test]
fn search_roots_use_the_exact_unsplit_coordinate_layout() {
    let temp = tempfile::tempdir().unwrap();
    let search_root = temp.path().join("search");
    let coordinate = ConeCoordinate::new("test.group", "dep.name", "1.0.0").unwrap();
    let artifact = search_root
        .join("test.group")
        .join("dep.name")
        .join("1.0.0")
        .join("cone.slib");
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(
        &artifact,
        manifest_artifact(coordinate.clone(), "locator-test"),
    )
    .unwrap();
    let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];

    let prebuilt = locate_from_search_roots(
        &coordinate,
        &roots,
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        prebuilt.first_candidate().resolved_path(),
        std::fs::canonicalize(artifact).unwrap()
    );
}

#[test]
fn absent_dangling_and_wrong_type_candidates_are_distinct() {
    let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let search_root = temp.path().join("search");
    let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];
    assert!(matches!(
        locate_from_search_roots(
            &coordinate,
            &roots,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::ArtifactNotFound { .. })
    ));

    let candidate = search_root
        .join("test")
        .join("dep")
        .join("1.0.0")
        .join("cone.slib");
    std::fs::create_dir_all(candidate.parent().unwrap()).unwrap();
    std::fs::create_dir(&candidate).unwrap();
    assert!(matches!(
        locate_from_search_roots(
            &coordinate,
            &roots,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::Io {
            operation: LocatorIoOperation::Open | LocatorIoOperation::Read,
            ..
        })
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let dangling_root = temp.path().join("dangling-search");
        let dangling = dangling_root
            .join("test")
            .join("dep")
            .join("1.0.0")
            .join("cone.slib");
        std::fs::create_dir_all(dangling.parent().unwrap()).unwrap();
        symlink(dangling_root.join("missing.slib"), &dangling).unwrap();
        let dangling_roots = [ArtifactSearchRoot::new(dangling_root).unwrap()];
        assert!(matches!(
            locate_from_search_roots(
                &coordinate,
                &dangling_roots,
                TARGET,
                &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
            ),
            Err(DependencyLocatorError::Io {
                operation: LocatorIoOperation::Canonicalize,
                ..
            })
        ));
    }
}

#[test]
fn search_root_candidates_require_one_complete_artifact_fingerprint() {
    let temp = tempfile::tempdir().unwrap();
    let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
    let roots = [temp.path().join("first"), temp.path().join("second")];
    let paths = roots.clone().map(|root| {
        root.join("test")
            .join("dep")
            .join("1.0.0")
            .join("cone.slib")
    });
    let bytes = manifest_artifact(coordinate.clone(), "same");
    for path in &paths {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &bytes).unwrap();
    }
    let search_roots = roots.map(ArtifactSearchRoot::new).map(Result::unwrap);
    let prebuilt = locate_from_search_roots(
        &coordinate,
        &search_roots,
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    assert_eq!(prebuilt.candidate_count(), 2);

    std::fs::write(
        &paths[1],
        manifest_artifact(coordinate.clone(), "different"),
    )
    .unwrap();
    assert!(matches!(
        locate_from_search_roots(
            &coordinate,
            &search_roots,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::AmbiguousArtifact { .. })
    ));
}
