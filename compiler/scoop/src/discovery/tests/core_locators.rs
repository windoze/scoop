use super::*;

fn write_core_at(root: &std::path::Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        include_str!("../../../../../sysroot/src/scoop.core/Cone.toml"),
    )
    .unwrap();
}

#[test]
fn transitive_core_source_locator_overrides_the_default_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let helper = temp.path().join("helper");
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
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
}

#[test]
fn explicit_core_artifact_and_search_root_use_common_prebuilt_claims() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
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
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
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
        matches!(request(&root, &sysroot, vec![]).load_root().unwrap().discover(), Err(BuildGraphDiscoveryError::ConflictingSourceLocator { coordinate, .. }) if coordinate == ConeCoordinate::reserved_core())
    );
}

#[test]
fn version_only_core_uses_the_default_slot_in_production_and_test_graphs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.core\" = \"0.1.0\"",
    );
    for purpose in [
        scoop_protocol::CompilationPurpose::Production,
        scoop_protocol::CompilationPurpose::Test,
    ] {
        let graph = request(&root, &sysroot, vec![])
            .with_purpose(purpose)
            .unwrap()
            .load_root()
            .unwrap()
            .discover()
            .unwrap();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert!(matches!(
            graph.edges().next().unwrap().origins(),
            [EdgeOrigin::ManifestDeclaration { .. }]
        ));
        assert_eq!(
            graph.resolve().unwrap().dependency_first()[0],
            ConeIdentity::CORE
        );
    }
}

#[test]
fn version_only_core_waits_for_default_libraries_explicit_core_locators() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let sysroot = temp.path().join("sysroot");
    write_core(&sysroot);
    let alternate = sysroot.join("alternate-core");
    write_core_at(&alternate);
    let helper = sysroot.join("src/scoop.helper");
    std::fs::create_dir_all(&helper).unwrap();
    std::fs::write(helper.join("Cone.toml"), "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.helper\"\nversion = \"1.0.0\"\nkind = \"library\"\n[dependencies]\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../../alternate-core\" }\n").unwrap();
    write_manifest(
        &root,
        "root",
        "[dependencies]\n\"scoop:scoop.core\" = \"0.1.0\"\n\"scoop:scoop.helper\" = \"1.0.0\"\n",
    );
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    let GraphNode::ManifestSource(core) = &graph.nodes[&ConeIdentity::CORE] else {
        panic!("the helper supplies the core source locator");
    };
    assert_eq!(core.real_root(), std::fs::canonicalize(alternate).unwrap());
    assert_eq!(
        graph.resolve().unwrap().dependency_first()[0],
        ConeIdentity::CORE
    );
}
