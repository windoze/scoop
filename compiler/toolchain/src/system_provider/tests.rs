use super::*;

#[test]
fn system_exports_follow_reexports_and_target_selection() {
    let sdk = tempfile::tempdir().unwrap();
    let root = sdk.path().join(LIBSYSTEM_STUB);
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    std::fs::write(
        &root,
        r#"--- !tapi-tbd
tbd-version: 4
targets: [arm64e-macos, x86_64-macos]
install-name: /usr/lib/libSystem.B.dylib
current-version: 1359.2.1
reexported-libraries:
  - targets: [arm64e-macos]
    libraries: [/usr/lib/system/leaf.dylib]
exports:
  - targets: [x86_64-macos]
    symbols: [_wrong_arch]
--- !tapi-tbd
tbd-version: 4
targets: [arm64-macos]
install-name: /usr/lib/system/leaf.dylib
exports:
  - targets: [arm64-macos]
    symbols: [_ordinary, '$ld$hide$os14.0$_ordinary', _unused_system_export]
    thread-local-symbols: [_tls]
"#,
    )
    .unwrap();
    let provider = SystemProvider::read(
        sdk.path(),
        DarwinPackedVersionV1::from_components(14, 0, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(provider.exports().len(), 2);
    assert_eq!(provider.current_version(), (1359 << 16) | (2 << 8) | 1);
    assert_eq!(provider.compatibility_version(), 1 << 16);
    assert_eq!(provider.exports()["_tls"], SystemExportKind::ThreadLocal);
    assert!(provider.exports().contains_key("_unused_system_export"));
    let destination = tempfile::tempdir().unwrap();
    let snapshot = provider.write_to(destination.path()).unwrap();
    assert_eq!(
        std::fs::read(root).unwrap(),
        std::fs::read(snapshot).unwrap()
    );
}

#[test]
fn missing_reexports_and_wrong_targets_report_the_actual_input() {
    let sdk = tempfile::tempdir().unwrap();
    let root = sdk.path().join(LIBSYSTEM_STUB);
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    let template = "--- !tapi-tbd\ntbd-version: 4\ntargets: [arm64-macos]\ninstall-name: /usr/lib/libSystem.B.dylib\nreexported-libraries:\n  - targets: [arm64-macos]\n    libraries: [/usr/lib/missing.dylib]\n";
    let deployment = DarwinPackedVersionV1::from_components(14, 0, 0).unwrap();
    std::fs::write(&root, template).unwrap();
    assert!(
        SystemProvider::read(sdk.path(), deployment)
            .unwrap_err()
            .to_string()
            .contains("missing.tbd")
    );
    std::fs::write(root, template.replace("arm64-macos", "x86_64-macos")).unwrap();
    assert!(
        SystemProvider::read(sdk.path(), deployment)
            .unwrap_err()
            .to_string()
            .contains("no compatible macOS/arm64")
    );
}
