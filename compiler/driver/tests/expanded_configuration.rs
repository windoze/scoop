#![cfg(unix)]

use std::{
    io::Write,
    process::{Command, Stdio},
};

use scoop_identity::ConeCoordinate;
use scoop_manifest::{ParsedConeManifest, package::parse_package_manifest};
use scoop_protocol::{package::ExpandedConeInputV1, *};

#[test]
fn child_checks_workspace_configuration_without_reinterpreting_the_manifest() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().join("core");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(workspace.path().join("Cone.toml"), "schema=1\n[workspace]\nmembers=['core']\n[workspace.cone]\ngroup='scoop'\nversion='0.1.0'\n").unwrap();
    let text = "schema=1\n[cone]\ngroup={workspace=true}\nname='scoop.core'\nversion={workspace=true}\nkind='library'\n";
    std::fs::write(root.join("Cone.toml"), text).unwrap();
    let sources = scoop_toolchain::development_workspace_root().join("sysroot/src/scoop.core/src");
    std::os::unix::fs::symlink(&sources, root.join("src")).unwrap();
    let document = parse_package_manifest(text, scoop_toolchain::SOURCE_TARGETS).unwrap();
    let manifest = ParsedConeManifest::bind_package(
        &document,
        ConeCoordinate::reserved_core(),
        &Default::default(),
        false,
    )
    .unwrap();
    let host = |path: &std::path::Path| HostPathCarrier::from_path(path).unwrap();
    let build = ScoopcBuildRequestV1::new(
        CurrentConeRequestV1::ExpandedCone {
            config: Box::new(ExpandedConeInputV1 {
                input_root: host(&root),
                root: host(&root),
                manifest_path: host(&root.join("Cone.toml")),
                manifest_text: text.into(),
                manifest,
                metadata: Default::default(),
            }),
        },
        Vec::new(),
        Vec::new(),
        TrustedCoreRequestV1::Bootstrap,
        TargetSelectionRequestV1::new("x86_64-unknown-linux-gnu".into()).unwrap(),
        ScoopcOutputRequestV1::Check,
        DiagnosticOutputPolicyV1::Structured,
        StageDumpPolicyV1::None,
    )
    .unwrap();
    let request = ScoopcRequestEnvelopeV1::new(RequestCorrelationId::from_array([18; 16]), build);
    let mut child = Command::new(env!("CARGO_BIN_EXE_scoopc"))
        .arg("__child-protocol")
        .arg(PROTOCOL_VERSION.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&encode_request_frame(&request).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = decode_response_frame(&output.stdout).unwrap();
    let ScoopcResponseEnvelopeV1::Success { request_id, result } = response else {
        panic!("{response:?}")
    };
    assert_eq!(request_id, request.request_id());
    assert!(matches!(result.product(), ScoopcProductV1::Check));
    assert!(result.warnings().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("Cone.toml")).unwrap(),
        text
    );
}
