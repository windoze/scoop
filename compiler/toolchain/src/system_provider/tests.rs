use super::*;

#[test]
fn previous_sdk_exports_follow_platform_and_half_open_deployment_ranges() {
    let text = b"--- !tapi-tbd\ntbd-version: 4\ntargets: [arm64-macos]\ninstall-name: /usr/lib/current.dylib\nexports:\n  - targets: [arm64-macos]\n    symbols: [_ordinary, '$ld$previous$/usr/lib/previous.dylib$3.1$1$12.0$14.0$_OBJC_CLASS_$_Moved$', '$ld$previous$/usr/lib/ios.dylib$1$2$1.0$99.0$_ios$']\n";
    for (major, active) in [(11, false), (12, true), (13, true), (14, false)] {
        let records = read_text_stubs(
            text,
            DarwinPackedVersionV1::from_components(major, 0, 0).unwrap(),
            true,
        )
        .unwrap();
        let record = &records[0];
        assert_eq!(record.previous_exports.len(), usize::from(active));
        if active {
            let previous = &record.previous_exports["_OBJC_CLASS_$_Moved"];
            assert_eq!(previous.install_name, "/usr/lib/previous.dylib");
            assert_eq!(previous.compatibility_version, (3 << 16) | (1 << 8));
        }
        assert!(record.exports.contains_key("_ordinary"));
        assert!(!record.exports.contains_key("_ios"));
    }
}

#[test]
fn previous_sdk_library_name_and_version_apply_without_a_symbol() {
    let text = b"--- !tapi-tbd\ntbd-version: 4\ntargets: [arm64-macos]\ninstall-name: /usr/lib/current.dylib\nexports:\n  - targets: [arm64-macos]\n    symbols: [_ordinary, '$ld$previous$/usr/lib/previous.dylib$3$1$12.0$14.0$$']\n";
    let records = read_text_stubs(
        text,
        DarwinPackedVersionV1::from_components(13, 0, 0).unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(records[0].install_name, "/usr/lib/previous.dylib");
    assert_eq!(records[0].compatibility_version, 3 << 16);
    assert!(records[0].previous_exports.is_empty());
}

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
    assert_eq!(
        provider.exports()["_tls"].kind,
        SystemExportKind::ThreadLocal
    );
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

#[test]
fn native_stubs_preserve_exports_and_reject_conflicting_install_records() {
    let deployment = DarwinPackedVersionV1::from_components(14, 0, 0).unwrap();
    let imports = BTreeMap::from([
        (
            "_ordinary".into(),
            NativeExport {
                kind: SystemExportKind::Symbol,
                weak: false,
            },
        ),
        (
            "_weak".into(),
            NativeExport {
                kind: SystemExportKind::Symbol,
                weak: true,
            },
        ),
        (
            "_tls".into(),
            NativeExport {
                kind: SystemExportKind::ThreadLocal,
                weak: false,
            },
        ),
    ]);
    let first = tbd::write_link_stub("@rpath/libtest.dylib", 2 << 16, 1 << 16, &imports).unwrap();
    let interface = tbd::read_text_stubs(&first, deployment, false).unwrap();
    assert_eq!(interface[0].exports, imports);
    let mut identical = first.clone();
    identical.extend_from_slice(&first);
    assert_eq!(
        tbd::read_text_stubs(&identical, deployment, false).unwrap(),
        interface
    );
    let mut conflicting = first;
    conflicting
        .extend(tbd::write_link_stub("@rpath/libtest.dylib", 3 << 16, 1 << 16, &imports).unwrap());
    assert!(
        tbd::read_text_stubs(&conflicting, deployment, false)
            .unwrap_err()
            .to_string()
            .contains("conflicting text stub records")
    );
}
