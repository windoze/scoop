use super::*;

#[test]
fn edited_core_rebuilds_through_the_common_cache_without_writing_sysroot_artifacts() {
    let Some(compiler) = std::env::var_os("SCOOP_TEST_PAIRED_SCOOPC") else {
        return;
    };
    let compiler = std::path::PathBuf::from(compiler);
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path();
    let sysroot = workspace.join("sysroot");
    let root = workspace.join("root");
    copy_real_core(&sysroot);
    let extension = sysroot.join("lib/scoop.core/src/extension.scoop");
    let source_v1 =
        include_str!("../../../../../../../tests/fixtures/core-library/extension.scoop");
    let source_v2 = source_v1.replace("+ 1", "+ 2");
    std::fs::write(&extension, source_v1).unwrap();
    write_manifest_source(
        &root,
        "root",
        "library",
        "public fun answer(): Int = userCoreOffset(41)\n",
        "",
    );
    let root_identity = ConeCoordinate::new("test", "root", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let prepare = || {
        real_manifest_request(&root, workspace, &sysroot, &compiler)
            .load_root()
            .unwrap()
            .discover()
            .unwrap()
            .resolve()
            .unwrap()
            .prepare()
            .unwrap()
    };
    let original = prepare();
    let old_key = original.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    let private_core = original.source_input_path(ConeIdentity::CORE).unwrap();
    std::fs::write(&extension, &source_v2).unwrap();
    assert_eq!(
        std::fs::read_to_string(private_core.join("src/extension.scoop")).unwrap(),
        source_v1
    );
    let first = original.execute().unwrap();
    assert_eq!(
        first.observations().child_invocations(),
        &[ConeIdentity::CORE, root_identity]
    );
    let changed = prepare();
    let new_key = changed.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
    assert_ne!(old_key, new_key);
    let second = changed.execute().unwrap();
    assert_eq!(
        second.observations().child_invocations(),
        &[ConeIdentity::CORE, root_identity]
    );
    for identity in [ConeIdentity::CORE, root_identity] {
        assert_eq!(
            second.completed(identity).unwrap().origin(),
            CompletedNodeOrigin::Compiled
        );
        assert_ne!(
            first
                .completed(identity)
                .unwrap()
                .artifact()
                .publication()
                .artifact_fingerprint(),
            second
                .completed(identity)
                .unwrap()
                .artifact()
                .publication()
                .artifact_fingerprint(),
        );
    }
    let third = prepare().execute().unwrap();
    assert!(third.observations().child_invocations().is_empty());
    for identity in [ConeIdentity::CORE, root_identity] {
        assert_eq!(
            third.completed(identity).unwrap().origin(),
            CompletedNodeOrigin::CacheHit
        );
        assert_eq!(
            second
                .completed(identity)
                .unwrap()
                .artifact()
                .snapshot()
                .as_bytes(),
            third
                .completed(identity)
                .unwrap()
                .artifact()
                .snapshot()
                .as_bytes(),
        );
    }
    let store = CompileCacheStoreV1::new(&ArtifactCacheRoot::new(workspace.join("cache")).unwrap());
    assert!(store.entry_path(old_key).join("receipt.cbor").is_file());
    assert!(store.entry_path(new_key).join("receipt.cbor").is_file());
    let layout = scoop_toolchain::TrustedCoreSlotLayoutV1::new(
        &sysroot,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    );
    assert!(!layout.artifact_root().exists());
}
