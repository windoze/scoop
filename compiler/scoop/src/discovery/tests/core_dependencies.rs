use super::*;

const CORE: &str =
    include_str!("../../../../../tests/fixtures/core-library/dependencies/core/Cone.toml");
const HELPER: &str =
    include_str!("../../../../../tests/fixtures/core-library/dependencies/helper/Cone.toml");

#[test]
fn core_dependency_cycle_uses_the_common_graph_and_preserves_edge_origins() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("core");
    let helper = temp.path().join("helper");
    let sysroot = temp.path().join("absent");
    for (path, source) in [(&root, CORE), (&helper, HELPER)] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("Cone.toml"), source).unwrap();
    }
    let graph = request(&root, &sysroot, vec![])
        .load_root()
        .unwrap()
        .discover()
        .unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 2);
    let error = graph.resolve().unwrap_err();
    assert_eq!(
        error.to_string(),
        "resolved graph contains dependency cycles; scoop:scoop.core:0.1.0 -> test:helper:1.0.0 -> scoop:scoop.core:0.1.0"
    );
    let crate::ResolveBuildGraphError::Cycles(cycles) = error else {
        panic!("core dependencies use the ordinary cycle diagnostic");
    };
    assert_eq!(cycles.len(), 1);
    let steps = cycles[0].steps();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].dependent(), ConeIdentity::CORE);
    assert_eq!(steps[1].dependency(), ConeIdentity::CORE);
    let [
        EdgeOrigin::ManifestDeclaration {
            manifest,
            span,
            locator_kind,
        },
    ] = steps[0].origins()
    else {
        panic!("the explicit core dependency retains its manifest location");
    };
    assert_eq!(
        manifest,
        &std::fs::canonicalize(root.join("Cone.toml")).unwrap()
    );
    assert_eq!(*locator_kind, ManifestLocatorKind::SourcePath);
    assert_eq!(
        &CORE[span.clone()],
        r#"{ version = "1.0.0", path = "../helper" }"#
    );
    assert_eq!(
        steps[1].origins(),
        &[EdgeOrigin::InjectedTrustedCore {
            dependent: steps[1].dependent()
        }]
    );
    assert!(!sysroot.exists());
    assert!(!root.join("src").exists());
}

#[test]
fn core_dependency_locator_failure_is_reported_before_source_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("core");
    let sysroot = temp.path().join("absent");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("Cone.toml"), CORE).unwrap();
    assert!(matches!(
        request(&root, &sysroot, vec![])
            .load_root()
            .unwrap()
            .discover(),
        Err(BuildGraphDiscoveryError::Locator(_))
    ));
    assert!(!sysroot.exists());
    assert!(!root.join("src").exists());
}
