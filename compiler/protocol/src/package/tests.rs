use super::*;

#[test]
fn json_paths_validate_encoding_and_preserve_non_utf8() {
    #[cfg(unix)]
    let path = {
        use std::os::unix::ffi::OsStringExt;
        std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', b'a', 0xff]))
    };
    #[cfg(windows)]
    let path = {
        use std::os::windows::ffi::OsStringExt;
        std::path::PathBuf::from(std::ffi::OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xd800,
            b'a' as u16,
        ]))
    };
    let carrier = HostPathCarrier::from_path(&path).unwrap();
    let encoded = serde_json::to_vec(&carrier).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert!(value["bytes"].is_string());
    let decoded: HostPathCarrier = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.to_path_buf().unwrap(), path);
    for value in [
        r#"{"encoding":"UnixBytes","bytes":""}"#,
        r#"{"encoding":"UnixBytes","bytes":"YQA="}"#,
        r#"{"encoding":"WindowsWtf16Le","bytes":"YQ=="}"#,
        r#"{"encoding":"UnixBytes","bytes":"%%%"}"#,
        r#"{"encoding":"UnixBytes","bytes":[97]}"#,
    ] {
        assert!(serde_json::from_str::<HostPathCarrier>(value).is_err());
    }
}

#[test]
fn request_requires_supported_version_absolute_cwd_and_nonempty_selection() {
    let cwd = HostPathCarrier::from_path(&std::env::current_dir().unwrap()).unwrap();
    let mut request = PrepareRequestV1 {
        version: PACKAGE_PROTOCOL_VERSION,
        cwd: cwd.clone(),
        input: None,
        members: PackageMemberSelectionV1::Default,
        purpose: CompilationPurpose::Production,
        target: "aarch64-apple-darwin".into(),
        sysroot: cwd,
        system_library_root: None,
        artifact_roots: Vec::new(),
        package_cache: None,
        lockfile: None,
        locked: false,
        offline: false,
    };
    request.validate().unwrap();
    let encoded = serde_json::to_vec(&request).unwrap();
    assert_eq!(
        serde_json::from_slice::<PrepareRequestV1>(&encoded).unwrap(),
        request
    );
    request.version = PACKAGE_PROTOCOL_VERSION + 1;
    assert!(
        request
            .validate()
            .unwrap_err()
            .contains("unsupported package protocol")
    );
    request.version = PACKAGE_PROTOCOL_VERSION;
    request.members = PackageMemberSelectionV1::Packages { keys: Vec::new() };
    assert!(
        request
            .validate()
            .unwrap_err()
            .contains("selection must not be empty")
    );
    request.members = PackageMemberSelectionV1::Default;
    request.cwd = HostPathCarrier::from_path(std::path::Path::new("relative")).unwrap();
    assert!(request.validate().unwrap_err().contains("must be absolute"));
}
