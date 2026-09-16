use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use scoop_protocol::{
    CurrentConeRequestV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
    HostPathCarrier, ProtocolArtifactFingerprint, ProtocolCodeFingerprint, ProtocolConeIdentity,
    ProtocolHirFingerprint, ProtocolLirFingerprint, ProtocolMirFingerprint,
    ProtocolRuntimeImageFingerprint, RequestCorrelationId, ScoopcBuildRequestV1,
    ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1, ScoopcSuccessV1, StageDumpPolicyV1,
    StructuredDiagnosticV1, TargetSelectionRequestV1, TrustedCoreRequestV1,
    encode_machine_capability_frame, encode_response_frame,
};

use super::*;
use crate::PairedScoopcLocator;

fn path(value: &str) -> HostPathCarrier {
    HostPathCarrier::from_path(Path::new(value)).unwrap()
}

fn request(id: [u8; 16]) -> ScoopcRequestEnvelopeV1 {
    ScoopcRequestEnvelopeV1::new(
        RequestCorrelationId::from_array(id),
        ScoopcBuildRequestV1::new(
            CurrentConeRequestV1::ManifestRoot {
                root: path("snapshot/Cone.toml"),
            },
            Vec::new(),
            Vec::new(),
            TrustedCoreRequestV1::ArtifactSlot {
                artifact: path("snapshot/core.slib"),
            },
            TargetSelectionRequestV1::new("aarch64-apple-darwin".to_owned()).unwrap(),
            path("output/candidate.slib"),
            DiagnosticOutputPolicyV1::Structured,
            StageDumpPolicyV1::None,
        )
        .unwrap(),
    )
}

fn success(id: [u8; 16]) -> ScoopcResponseEnvelopeV1 {
    ScoopcResponseEnvelopeV1::success(
        RequestCorrelationId::from_array(id),
        ScoopcSuccessV1::new(
            ProtocolArtifactFingerprint::from_array([1; 32]),
            ProtocolConeIdentity::from_array([2; 32]),
            ProtocolHirFingerprint::from_array([3; 32]),
            ProtocolMirFingerprint::from_array([4; 32]),
            ProtocolLirFingerprint::from_array([5; 32]),
            ProtocolCodeFingerprint::from_array([6; 32]),
            ProtocolRuntimeImageFingerprint::from_array([7; 32]),
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
    )
}

fn failure(id: [u8; 16]) -> ScoopcResponseEnvelopeV1 {
    let diagnostic = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOPC_TEST_FAILURE".to_owned(),
        "test failure".to_owned(),
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .unwrap();
    ScoopcResponseEnvelopeV1::failure(RequestCorrelationId::from_array(id), vec![diagnostic])
        .unwrap()
}

fn shell_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{:03o}", byte)).collect()
}

fn fake_compiler(path: &Path, response: &[u8], exit: i32, stderr: &str) {
    let capability = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    let capability = shell_bytes(&encode_machine_capability_frame(&capability).unwrap());
    let response = shell_bytes(response);
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"__machine-capability\" ]; then\n  printf '{capability}'\n  exit 0\nfi\n/bin/cat >/dev/null\nprintf '{response}'\nprintf '%s' '{stderr}' >&2\nexit {exit}\n"
    );
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn resolve_fake(path: &Path) -> ResolvedPairedScoopc {
    ResolvedPairedScoopc::resolve(&PairedScoopcLocator::new(path).unwrap()).unwrap()
}

#[test]
fn production_runner_accepts_only_matching_response_and_exit_pairs() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let request = request([8; 16]);
    let io = ChildIoPlan::new(directory.path().to_path_buf());
    fake_compiler(
        &compiler,
        &encode_response_frame(&success([8; 16])).unwrap(),
        0,
        "",
    );
    let tool = resolve_fake(&compiler);

    let response = ProductionSingleConeCompilerRunner
        .invoke(&tool, &request, &io)
        .unwrap();
    assert!(matches!(response, ScoopcResponseEnvelopeV1::Success { .. }));

    fake_compiler(
        &compiler,
        &encode_response_frame(&failure([8; 16])).unwrap(),
        COMPILER_FAILURE_EXIT_CODE,
        "",
    );
    let tool = resolve_fake(&compiler);
    let response = ProductionSingleConeCompilerRunner
        .invoke(&tool, &request, &io)
        .unwrap();
    assert!(matches!(response, ScoopcResponseEnvelopeV1::Failure { .. }));
}

#[test]
fn production_runner_rejects_wrong_id_exit_stderr_and_trailing_frame() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let request = request([8; 16]);
    let io = ChildIoPlan::new(directory.path().to_path_buf());

    fake_compiler(
        &compiler,
        &encode_response_frame(&success([9; 16])).unwrap(),
        0,
        "",
    );
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::RequestIdMismatch { .. })
    ));

    fake_compiler(
        &compiler,
        &encode_response_frame(&success([8; 16])).unwrap(),
        COMPILER_FAILURE_EXIT_CODE,
        "",
    );
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::ExitMismatch {
            response: ChildResponseKind::Success,
            ..
        })
    ));

    fake_compiler(
        &compiler,
        &encode_response_frame(&success([8; 16])).unwrap(),
        0,
        "fatal detail",
    );
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::UnexpectedStderr(_))
    ));

    let mut trailing = encode_response_frame(&success([8; 16])).unwrap();
    trailing.push(0);
    fake_compiler(&compiler, &trailing, 0, "");
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::Response { .. })
    ));
}
