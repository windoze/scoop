use super::*;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::ConeRecord;
use scoop_toolchain::load_cone_manifest;

const TARGET: ValidatedLirTargetSelection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;

fn write_manifest(path: &Path, name: &str, kind: &str, dependency: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
            path.join("Cone.toml"),
            format!(
                "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"{kind}\"\n{dependency}"
            ),
        )
        .unwrap();
}

fn parent_with_dependency(root: &Path, dependency: &str) -> LoadedConeManifest {
    write_manifest(root, "root", "library", dependency);
    load_cone_manifest(&ManifestRootLocator::cone_directory(root)).unwrap()
}

fn first_key(manifest: &LoadedConeManifest) -> DependencyCoordinateKey {
    manifest
        .parsed()
        .semantic()
        .dependency_iter()
        .next()
        .unwrap()
        .0
        .clone()
}

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
fn source_path_resolves_from_the_real_declaring_root() {
    let temp = tempfile::tempdir().unwrap();
    let parent_root = temp.path().join("parent");
    let dependency_root = temp.path().join("dependency");
    write_manifest(&dependency_root, "dep", "library", "");
    let parent = parent_with_dependency(
        &parent_root,
        "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
    );

    let claim = locate_manifest_dependency(
        &parent,
        &first_key(&parent),
        &[],
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    let LocatedDependencyClaim::Source(source) = claim else {
        panic!("source locator must produce a source claim")
    };
    assert_eq!(source.manifest().coordinate().name(), "dep");
    assert_eq!(
        source.manifest().real_root(),
        std::fs::canonicalize(dependency_root).unwrap()
    );
}

#[test]
fn source_coordinate_and_kind_are_checked_at_the_locator_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let parent_root = temp.path().join("parent");
    let dependency_root = temp.path().join("dependency");
    write_manifest(&dependency_root, "other", "library", "");
    let parent = parent_with_dependency(
        &parent_root,
        "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", path = \"../dependency\" }\n",
    );
    assert!(matches!(
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
            &[],
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::CoordinateMismatch { .. })
    ));

    write_manifest(&dependency_root, "dep", "executable", "");
    assert!(matches!(
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
            &[],
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::ExecutableDependency { .. })
    ));
}

#[test]
fn explicit_artifact_is_probed_without_becoming_a_compile_or_link_proof() {
    let temp = tempfile::tempdir().unwrap();
    let parent_root = temp.path().join("parent");
    let artifact = temp.path().join("dep.slib");
    let coordinate = ConeCoordinate::new("test", "dep", "1.0.0").unwrap();
    let bytes = manifest_artifact(coordinate.clone(), "locator-test");
    std::fs::write(&artifact, &bytes).unwrap();
    let parent = parent_with_dependency(
        &parent_root,
        "[dependencies]\n\"test:dep\" = { version = \"1.0.0\", artifact = \"../dep.slib\" }\n",
    );

    let claim = locate_manifest_dependency(
        &parent,
        &first_key(&parent),
        &[],
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    let LocatedDependencyClaim::Prebuilt(prebuilt) = claim else {
        panic!("artifact locator must produce a prebuilt claim")
    };
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
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
            &[],
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
    let parent_root = temp.path().join("parent");
    let search_root = temp.path().join("search");
    let coordinate = ConeCoordinate::new("test.group", "dep.name", "1.0.0").unwrap();
    let artifact = search_root
        .join("test.group")
        .join("dep.name")
        .join("1.0.0")
        .join("cone.slib");
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(&artifact, manifest_artifact(coordinate, "locator-test")).unwrap();
    let parent = parent_with_dependency(
        &parent_root,
        "[dependencies]\n\"test.group:dep.name\" = \"1.0.0\"\n",
    );
    let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];

    let claim = locate_manifest_dependency(
        &parent,
        &first_key(&parent),
        &roots,
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap();
    let LocatedDependencyClaim::Prebuilt(prebuilt) = claim else {
        panic!("search root must produce a prebuilt claim")
    };
    assert_eq!(
        prebuilt.first_candidate().resolved_path(),
        std::fs::canonicalize(artifact).unwrap()
    );
}

#[test]
fn absent_dangling_and_wrong_type_candidates_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let parent_root = temp.path().join("parent");
    let parent = parent_with_dependency(&parent_root, "[dependencies]\n\"test:dep\" = \"1.0.0\"\n");
    let search_root = temp.path().join("search");
    let roots = [ArtifactSearchRoot::new(&search_root).unwrap()];
    assert!(matches!(
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
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
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
            &roots,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::ArtifactNotRegularFile(_))
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
            locate_manifest_dependency(
                &parent,
                &first_key(&parent),
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
    let parent_root = temp.path().join("parent");
    let parent = parent_with_dependency(&parent_root, "[dependencies]\n\"test:dep\" = \"1.0.0\"\n");
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
    let LocatedDependencyClaim::Prebuilt(prebuilt) = locate_manifest_dependency(
        &parent,
        &first_key(&parent),
        &search_roots,
        TARGET,
        &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap(),
    )
    .unwrap() else {
        panic!("search root must produce a prebuilt claim")
    };
    assert_eq!(prebuilt.candidate_count(), 2);

    std::fs::write(&paths[1], manifest_artifact(coordinate, "different")).unwrap();
    assert!(matches!(
        locate_manifest_dependency(
            &parent,
            &first_key(&parent),
            &search_roots,
            TARGET,
            &ArtifactCacheRoot::new(temp.path().join("cache")).unwrap()
        ),
        Err(DependencyLocatorError::AmbiguousArtifact { .. })
    ));
}
