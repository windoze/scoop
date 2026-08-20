use std::path::{Path, PathBuf};

use scoop::{
    ArtifactCacheRoot, BuildGraphRequest, BuildRootInput, DiagnosticsPolicy, PairedScoopcLocator,
    TrustedSysrootRoot, read_built_program,
};
use scoop_identity::ConeIdentity;
use scoop_manifest::ManifestRootLocator;
use scoop_protocol::TargetSelectionRequestV1;

#[test]
fn completed_graph_links_retained_artifacts_after_original_files_are_removed() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiler = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("scoopc")
        });
    assert!(
        compiler.is_file(),
        "build the paired scoopc before program-link tests"
    );
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    std::fs::create_dir_all(source.join("src")).unwrap();
    std::fs::copy(
        workspace.join("tests/fixtures/m23-cli-program-link/retained-artifacts/cone/Cone.toml"),
        source.join("Cone.toml"),
    )
    .unwrap();
    std::fs::copy(
        workspace.join("tests/fixtures/m23-program-link/read-println.scoop"),
        source.join("src/main.scoop"),
    )
    .unwrap();
    let request = BuildGraphRequest::new(
        BuildRootInput::manifest(ManifestRootLocator::cone_directory(&source)).unwrap(),
        vec![],
        ArtifactCacheRoot::new(directory.path().join("cache")).unwrap(),
        TrustedSysrootRoot::new(workspace.join("sysroot")).unwrap(),
        TargetSelectionRequestV1::new(scoop_toolchain::host_target_triple().unwrap().into())
            .unwrap(),
        PairedScoopcLocator::new(compiler).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap();
    let target = request.target().clone();
    let graph = request
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
        .execute()
        .unwrap()
        .into_outcome();
    assert_eq!(graph.artifacts().len(), 2);
    assert_eq!(graph.artifacts()[0].cone(), ConeIdentity::CORE);

    // Delete only after graph execution completes and its staging is dropped.
    std::fs::remove_dir_all(source).unwrap();
    for node in graph.artifacts() {
        assert!(node.artifact_locator().is_file());
        std::fs::remove_file(node.artifact_locator()).unwrap();
        assert!(!node.materialized_child_path().as_path().exists());
    }
    let closure = read_built_program(graph.closure(), &target).unwrap();
    assert_eq!(closure.root(), graph.root().cone());
    assert_eq!(closure.artifacts().len(), graph.artifacts().len());
    for ((linked, _), retained) in closure.artifacts().zip(graph.artifacts()) {
        assert_eq!(linked.identity(), retained.cone());
        assert_eq!(
            linked.manifest().artifact_fingerprint(),
            retained.artifact().summary().artifact_fingerprint()
        );
    }
}
