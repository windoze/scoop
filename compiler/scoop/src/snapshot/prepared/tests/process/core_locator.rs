use super::*;

#[test]
fn explicit_core_source_rebuilds_and_can_be_replaced_by_its_prebuilt_artifact() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let distribution = workspace.join("distribution");
    copy_real_core(&distribution);
    let core_root = workspace.join("core");
    std::fs::rename(distribution.join("lib/scoop.core"), &core_root).unwrap();
    let extension = core_root.join("src/extension.scoop");
    let source = include_str!("../../../../../../../tests/fixtures/core-library/extension.scoop");
    std::fs::write(&extension, source).unwrap();
    let root = workspace.join("root");
    write_manifest(&workspace.join("helper"), "helper", "");
    write_manifest_source(
        &root,
        "root",
        "library",
        include_str!("../../../../../../../tests/fixtures/core-library/library-consumer.scoop"),
        "[dependencies]\n\"scoop:scoop.core\" = { version = \"0.1.0\", path = \"../core\" }\n\"test:helper\" = { version = \"1.0.0\", path = \"../helper\" }",
    );
    let root_id = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let helper_id = ConeCoordinate::new("test", "helper", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let sysroot = workspace.join("absent-sysroot");
    let build = || {
        real_manifest_request(&root, workspace, &sysroot, &compiler)
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
        &[ConeIdentity::CORE, helper_id, root_id]
    );
    let warm = build();
    assert!(warm.observations().child_invocations().is_empty());
    std::fs::write(&extension, source.replace("+ 1", "+ 2")).unwrap();
    let changed = build();
    assert_eq!(
        changed.observations().child_invocations(),
        &[ConeIdentity::CORE, helper_id, root_id]
    );
    for identity in [ConeIdentity::CORE, helper_id, root_id] {
        assert_ne!(
            first
                .completed(identity)
                .unwrap()
                .artifact()
                .publication()
                .artifact_fingerprint(),
            changed
                .completed(identity)
                .unwrap()
                .artifact()
                .publication()
                .artifact_fingerprint()
        );
    }
    let core = changed.completed(ConeIdentity::CORE).unwrap();
    std::fs::write(
        workspace.join("core.slib"),
        core.artifact().snapshot().as_bytes(),
    )
    .unwrap();
    let manifest = root.join("Cone.toml");
    std::fs::write(
        &manifest,
        std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("path = \"../core\"", "artifact = \"../core.slib\""),
    )
    .unwrap();
    std::fs::remove_dir_all(&core_root).unwrap();
    let prebuilt = build();
    assert!(prebuilt.observations().child_invocations().is_empty());
    assert_eq!(
        prebuilt.completed(ConeIdentity::CORE).unwrap().origin(),
        CompletedNodeOrigin::Prebuilt
    );
    for identity in [helper_id, root_id] {
        let node = prebuilt.completed(identity).unwrap();
        assert_eq!(node.origin(), CompletedNodeOrigin::CacheHit);
        assert_eq!(
            node.artifact().snapshot().as_bytes(),
            changed
                .completed(identity)
                .unwrap()
                .artifact()
                .snapshot()
                .as_bytes()
        );
    }
    assert!(!sysroot.exists());
}
