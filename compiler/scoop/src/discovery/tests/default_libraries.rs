use super::*;

fn write_platform(root: &std::path::Path, dependencies: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        format!(
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.platform\"\n\
             version = \"0.1.0\"\nkind = \"library\"\n{dependencies}"
        ),
    )
    .unwrap();
}

#[test]
fn default_library_uses_artifacts_and_keeps_sources_outside_the_graph() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.platform\" = \"0.1.0\"\n",
    );
    write_platform(
        &sysroot.join("src/scoop.platform"),
        "[dependencies]\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }\n",
    );
    write_manifest(&sysroot.join("src/helper"), "helper", "");
    let selection = crate::test_artifacts::host_target();
    let directory = scoop_toolchain::target_library_root(&sysroot, selection.target());
    let core = scoop_slib::read_artifact_manifest_summary(
        &std::fs::read(directory.join("scoop.core.slib")).unwrap(),
        selection,
    )
    .unwrap()
    .semantic_fingerprints();
    let coordinate = ConeCoordinate::new("scoop", "scoop.platform", "0.1.0").unwrap();
    let artifact = crate::test_artifacts::manifest_archive(
        selection,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            coordinate.clone(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "precompiled-platform",
        vec![
            scoop_slib::DependencyRecord::new(
                ConeCoordinate::reserved_core(),
                core.hir(),
                core.mir(),
                core.lir(),
            )
            .unwrap(),
        ],
    );
    std::fs::write(directory.join("scoop.platform.slib"), artifact.as_bytes()).unwrap();
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(
        graph.node_representation(coordinate.identity().unwrap()),
        Some(DiscoveredNodeRepresentation::PrebuiltArtifact)
    );
    graph.resolve().unwrap();
}

#[test]
fn explicit_source_does_not_read_the_default_library() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let default = sysroot.join("src/scoop.platform");
    std::fs::create_dir_all(&default).unwrap();
    std::fs::write(default.join("Cone.toml"), "invalid manifest").unwrap();
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.platform\" = { version = \"0.1.0\", path = \"../selected\" }\n",
    );
    write_platform(&temp.path().join("selected"), "");
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 3);
}

#[test]
fn explicit_artifact_search_precedes_default_library_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let default = sysroot.join("src/scoop.platform");
    std::fs::create_dir_all(&default).unwrap();
    std::fs::write(default.join("Cone.toml"), "invalid manifest").unwrap();
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.platform\" = \"0.1.0\"\n",
    );
    let coordinate = ConeCoordinate::new("scoop", "scoop.platform", "0.1.0").unwrap();
    let directory = scoop_toolchain::target_library_root(
        &sysroot,
        crate::test_artifacts::host_target().target(),
    );
    std::fs::write(
        directory.join("scoop.platform.slib"),
        manifest_artifact(coordinate.clone(), "default"),
    )
    .unwrap();
    let search = temp.path().join("artifacts");
    let artifact = crate::locator::artifact_candidate_path(&search, &coordinate);
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(&artifact, manifest_artifact(coordinate.clone(), "platform")).unwrap();
    let graph = request(
        &root,
        &sysroot,
        vec![ArtifactSearchRoot::new(search).unwrap()],
    )
    .load_root()
    .unwrap()
    .discover()
    .unwrap();
    assert_eq!(
        graph.node_representation(coordinate.identity().unwrap()),
        Some(DiscoveredNodeRepresentation::PrebuiltArtifact)
    );
}

#[test]
fn missing_default_artifact_reports_target_coordinate_and_path_with_sources_present() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    write_platform(&sysroot.join("src/scoop.platform"), "");
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.platform\" = \"0.1.0\"\n",
    );
    let failure = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap_err();
    let message = failure.to_string();
    assert!(message.contains("scoop:scoop.platform:0.1.0"));
    assert!(message.contains(scoop_toolchain::host_target_triple().unwrap()));
    assert!(message.contains("scoop.platform.slib"));
    assert_eq!(
        crate::ClassifyBuildFailure::classification(&failure).code(),
        Some(crate::BuildDiagnosticCode::LOCATOR_NOT_FOUND),
    );
}

#[test]
fn other_groups_do_not_search_the_default_library_directory() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    write_manifest(&sysroot.join("src/scoop.platform"), "scoop.platform", "");
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:scoop.platform\" = \"1.0.0\"\n",
    );
    let error = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap_err();
    assert!(matches!(
        error,
        BuildGraphDiscoveryError::Locator(source)
            if matches!(*source, DependencyLocatorError::ArtifactNotFound { .. })
    ));
}
