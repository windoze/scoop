use super::*;
use scoop::{
    ArtifactCacheRoot, BuildGraphRequest, BuildRootInput, DiagnosticsPolicy, PairedScoopcLocator,
    TrustedSysrootRoot, link_built_program,
};
use scoop_linker::RuntimeObjectSet;
use scoop_manifest::ManifestRootLocator;
use scoop_protocol::TargetSelectionRequestV1;
use scoop_toolchain::ValidatedFinalLinkProfile;

#[test]
fn orchestration_links_retained_snapshots_after_source_and_cache_removal() {
    let environment = environment();
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("build");
    let root = workspace.join("root");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::copy(&environment.core, workspace.join("core.slib")).unwrap();
    std::fs::write(
        root.join("Cone.toml"),
        r#"schema = 1
[cone]
group = "dev.programlink"
name = "orchestrated"
version = "0.1.0"
kind = "executable"
[dependencies]
"scoop:scoop.core" = { version = "0.1.0", artifact = "../core.slib" }
"#,
    )
    .unwrap();
    std::fs::write(root.join("src/main.scoop"), fixture("read-println.scoop")).unwrap();
    let outcome = BuildGraphRequest::new(
        BuildRootInput::manifest(ManifestRootLocator::cone_directory(&root)).unwrap(),
        vec![],
        ArtifactCacheRoot::new(workspace.join("cache")).unwrap(),
        TrustedSysrootRoot::new(workspace.join("absent-sysroot")).unwrap(),
        TargetSelectionRequestV1::new("aarch64-apple-darwin".into()).unwrap(),
        PairedScoopcLocator::new(&environment.compiler).unwrap(),
        DiagnosticsPolicy::Structured,
    )
    .unwrap()
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
    assert_eq!(outcome.observations().child_invocations().len(), 1);
    let root_artifact = directory.path().join("root.slib");
    std::fs::write(
        &root_artifact,
        outcome.root().artifact().snapshot().as_bytes(),
    )
    .unwrap();
    std::fs::remove_dir_all(workspace).unwrap();
    assert!(!outcome.root().materialized_child_path().as_path().exists());

    let disk = environment.link(&root_artifact, &[], &directory.path().join("disk-program"));
    std::fs::remove_file(root_artifact).unwrap();
    let profile = ValidatedFinalLinkProfile::resolve("aarch64-apple-darwin").unwrap();
    let runtime = RuntimeObjectSet::read_index(
        &environment.runtime_index,
        profile.target(),
        profile.startup_toolchain().profile(),
    )
    .unwrap();
    let output = link_built_program(
        outcome.closure(),
        &runtime,
        &profile,
        &[],
        &directory.path().join("memory-program"),
    )
    .unwrap();
    assert!(
        disk.contains(&format!("link-plan {}", output.fingerprint)),
        "{disk}"
    );
    assert_eq!(run(&output.path, false), "42\n");
    assert_eq!(run(&output.path, true), "42\n");
}
