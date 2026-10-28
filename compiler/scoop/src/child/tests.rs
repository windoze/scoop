use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use scoop_protocol::{
    CurrentConeRequestV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
    HostPathCarrier, RequestCorrelationId, ScoopcBuildRequestV1, ScoopcRequestEnvelopeV1,
    ScoopcResponseEnvelopeV1, ScoopcSuccessV1, StageDumpPolicyV1, StructuredDiagnosticV1,
    TargetSelectionRequestV1, TrustedCoreRequestV1, encode_machine_capability_frame,
    encode_response_frame,
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
            CurrentConeRequestV1::SingleFile {
                source: path("snapshot/main.scoop"),
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
            scoop_slib::read_artifact_manifest_summary(
                crate::test_artifacts::manifest_archive(
                    scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
                    scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
                    scoop_slib::ConeRecord::new(
                        scoop_identity::ConeCoordinate::reserved_core(),
                        scoop_slib::ConeKind::Library,
                        scoop_slib::ConeSourceForm::Manifest,
                    )
                    .unwrap(),
                    "child-transport",
                    Vec::new(),
                )
                .as_bytes(),
                scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
            )
            .unwrap(),
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
    let response = shell_bytes(response);
    fake_compiler_body(
        path,
        &format!(
            "/bin/cat >/dev/null\nprintf '{response}'\nprintf '%s' '{stderr}' >&2\nexit {exit}"
        ),
    );
}

fn fake_compiler_body(path: &Path, body: &str) {
    let capability = scoop_toolchain::paired_compiler_machine_capability().unwrap();
    let capability = shell_bytes(&encode_machine_capability_frame(&capability).unwrap());
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"__machine-capability\" ]; then\n  printf '{capability}'\n  exit 0\nfi\n{body}\n"
    );
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn resolve_fake(path: &Path) -> ResolvedPairedScoopc {
    ResolvedPairedScoopc::resolve(
        &PairedScoopcLocator::new(path).unwrap(),
        &crate::ArtifactCacheRoot::new(path.parent().unwrap().join("cache")).unwrap(),
    )
    .unwrap()
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

#[cfg(target_os = "linux")]
#[test]
fn production_runner_preserves_linux_tools_and_uses_private_scratch() {
    for triple in ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"] {
        let target = scoop_toolchain::ResolvedTargetProfile::resolve(triple).unwrap();
        let selection = target.child_request().unwrap();
        assert!(selection.resolved_toolchain().is_some());
        assert_eq!(
            scoop_toolchain::ResolvedTargetProfile::resolve_request(&selection).unwrap(),
            target
        );
        let directory = tempfile::tempdir().unwrap();
        let compiler = directory.path().join("scoopc");
        let response = shell_bytes(&encode_response_frame(&success([8; 16])).unwrap());
        let gcc_check = if triple.ends_with("-musl") {
            "test -x \"$REALGCC\" || exit 9\n\"$REALGCC\" -dumpmachine >/dev/null || exit 9"
        } else {
            "test -z \"${REALGCC+x}\" || exit 9"
        };
        fake_compiler_body(
            &compiler,
            &format!(
                "/bin/cat >/dev/null\ntest -n \"$PATH\" || exit 9\n{gcc_check}\ncase \"$TMPDIR\" in \"$4\"/.scoop-child-*) ;; *) exit 9 ;; esac\ntest -d \"$TMPDIR\" || exit 9\nprintf '{response}'"
            ),
        );
        let io = ChildIoPlan::new(directory.path().to_owned())
            .with_toolchain(target.c_bridge_toolchain());
        ProductionSingleConeCompilerRunner
            .invoke(&resolve_fake(&compiler), &request([8; 16]), &io)
            .unwrap();
        assert!(!std::fs::read_dir(directory.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".scoop-child-")
        }));
    }
}

#[test]
fn production_runner_rejects_wrong_id_exit_and_trailing_frame() {
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

    let mut trailing = encode_response_frame(&success([8; 16])).unwrap();
    trailing.push(0);
    fake_compiler(&compiler, &trailing, 0, "");
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::Response { .. })
    ));
}

#[test]
fn production_runner_rejects_failure_with_zero_exit_and_human_stdout() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let request = request([8; 16]);
    let io = ChildIoPlan::new(directory.path().to_path_buf());

    fake_compiler(
        &compiler,
        &encode_response_frame(&failure([8; 16])).unwrap(),
        0,
        "",
    );
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::ExitMismatch {
            response: ChildResponseKind::Failure,
            ..
        })
    ));

    fake_compiler(&compiler, b"human compiler output\n", 0, "");
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::Response { .. })
    ));
}

#[test]
fn production_runner_reports_signal() {
    let directory = tempfile::tempdir().unwrap();
    let compiler = directory.path().join("scoopc");
    let request = request([8; 16]);
    let io = ChildIoPlan::new(directory.path().to_path_buf());

    fake_compiler_body(&compiler, "/bin/cat >/dev/null\nkill -TERM $$");
    let tool = resolve_fake(&compiler);
    assert!(matches!(
        ProductionSingleConeCompilerRunner.invoke(&tool, &request, &io),
        Err(ChildTransportError::Signal)
    ));
}
