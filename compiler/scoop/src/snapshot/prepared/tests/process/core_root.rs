use super::*;

#[test]
fn real_process_builds_and_caches_an_edited_core_root_without_a_sysroot() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let source = workspace.join("distribution");
    copy_real_core(&source);
    let root = workspace.join("edited-library");
    std::fs::rename(source.join("lib/scoop.core"), &root).unwrap();
    std::fs::write(
        root.join("src/extension.scoop"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/core-library/extension.scoop"
        )),
    )
    .unwrap();
    let missing_sysroot = workspace.join("missing-sysroot");
    let build = || {
        real_manifest_request(&root, workspace, &missing_sysroot, &compiler)
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
    };
    let first = build();
    assert_eq!(
        first.observations().child_invocations(),
        &[ConeIdentity::CORE]
    );
    assert_eq!(
        first.completed(ConeIdentity::CORE).unwrap().origin(),
        CompletedNodeOrigin::Compiled
    );
    let first = first.into_outcome();
    assert!(matches!(first, BuildGraphOutcome::Library { .. }));
    let second = build();
    assert!(second.observations().child_invocations().is_empty());
    assert_eq!(
        second.completed(ConeIdentity::CORE).unwrap().origin(),
        CompletedNodeOrigin::CacheHit
    );
    assert_eq!(
        first.root().artifact().snapshot().as_bytes(),
        second
            .completed(ConeIdentity::CORE)
            .unwrap()
            .artifact()
            .snapshot()
            .as_bytes()
    );
    assert!(!missing_sysroot.exists());
}
