use super::*;

fn write_core_at(root: &std::path::Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        include_str!("../../../../../sysroot/lib/scoop.core/Cone.toml"),
    )
    .unwrap();
}

#[test]
fn transitive_core_source_locator_supplies_implicit_edges_without_a_sysroot() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let helper = temp.path().join("helper");
    let sysroot = temp.path().join("absent");
    write_core_at(&temp.path().join("core"));
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }",
    );
    write_manifest(
        &helper,
        "helper",
        "[dependencies]\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../core\" }",
    );
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(
        graph.node_representation(ConeIdentity::CORE),
        Some(DiscoveredNodeRepresentation::ManifestSource)
    );
    let helper_id = ConeCoordinate::new("test", "helper", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let edge = graph
        .edges()
        .find(|edge| edge.dependent() == helper_id)
        .unwrap();
    assert!(matches!(
        edge.origins(),
        [EdgeOrigin::ManifestDeclaration { .. }]
    ));
    let resolved = graph.resolve().unwrap();
    assert_eq!(resolved.dependency_first()[0], ConeIdentity::CORE);
    assert!(!sysroot.exists());
}

#[test]
fn explicit_core_artifact_and_search_root_use_common_prebuilt_claims() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("absent");
    let search_root = temp.path().join("artifacts");
    let directory = search_root.join("scoop/scoop.core/0.1.0");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("cone.slib"),
        manifest_artifact(ConeCoordinate::reserved_core(), "core"),
    )
    .unwrap();
    for locator in [
        "{ version = \"0.1.0\", artifact = \"../artifacts/scoop/scoop.core/0.1.0/cone.slib\" }",
        "\"0.1.0\"",
    ] {
        write_manifest(
            &root,
            "root",
            &format!("[dependencies]\n\"scoop:scoop.core\" = {locator}"),
        );
        let graph = request(
            &root,
            &sysroot,
            vec![ArtifactSearchRoot::new(&search_root).unwrap()],
        )
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(
            graph.node_representation(ConeIdentity::CORE),
            Some(DiscoveredNodeRepresentation::PrebuiltArtifact)
        );
        assert_eq!(
            graph.resolve().unwrap().dependency_first()[0],
            ConeIdentity::CORE
        );
    }
    assert!(!sysroot.exists());
}

#[test]
fn explicit_core_locator_errors_do_not_fall_back_to_default_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../missing\" }",
    );
    assert!(matches!(
        request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover(),
        Err(BuildGraphDiscoveryError::Locator(_))
    ));
    write_manifest(&temp.path().join("missing"), "different", "");
    let error = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap_err();
    assert!(
        matches!(error, BuildGraphDiscoveryError::Locator(source) if matches!(source.as_ref(), DependencyLocatorError::CoordinateMismatch { .. }))
    );
}

#[test]
fn distinct_explicit_core_sources_report_the_common_locator_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let helper = temp.path().join("helper");
    write_core_at(&temp.path().join("core-a"));
    write_core_at(&temp.path().join("core-b"));
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../core-a\" }",
    );
    write_manifest(
        &helper,
        "helper",
        "[dependencies]\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../core-b\" }",
    );
    assert!(
        matches!(request(&root, &temp.path().join("absent"), vec![]).load_root().unwrap().discover(), Err(BuildGraphDiscoveryError::ConflictingSourceLocator { coordinate, .. }) if coordinate == ConeCoordinate::reserved_core())
    );
}
