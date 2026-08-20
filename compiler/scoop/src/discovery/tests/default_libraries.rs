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
fn default_library_expands_new_explicit_dependencies() {
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
        &sysroot.join("lib/scoop.platform"),
        "[dependencies]\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }\n",
    );
    write_manifest(&sysroot.join("lib/helper"), "helper", "");
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 5);
    assert_eq!(
        graph.node_representation(
            ConeCoordinate::new("test", "helper", "1.0.0")
                .unwrap()
                .identity()
                .unwrap()
        ),
        Some(DiscoveredNodeRepresentation::ManifestSource)
    );
    graph.resolve().unwrap();
}

#[test]
fn explicit_source_does_not_read_the_default_library() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let default = sysroot.join("lib/scoop.platform");
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
fn explicit_artifact_search_precedes_default_library_source() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let default = sysroot.join("lib/scoop.platform");
    std::fs::create_dir_all(&default).unwrap();
    std::fs::write(default.join("Cone.toml"), "invalid manifest").unwrap();
    let root = temp.path().join("root");
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.platform\" = \"0.1.0\"\n",
    );
    let coordinate = ConeCoordinate::new("scoop", "scoop.platform", "0.1.0").unwrap();
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
fn other_groups_do_not_search_the_default_library_directory() {
    let temp = tempfile::tempdir().unwrap();
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    write_manifest(&sysroot.join("lib/scoop.platform"), "scoop.platform", "");
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
