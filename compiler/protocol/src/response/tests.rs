use super::*;
use crate::{DiagnosticOriginV1, DiagnosticSeverityV1};

fn diagnostic(severity: DiagnosticSeverityV1, message: String) -> StructuredDiagnosticV1 {
    StructuredDiagnosticV1::new(
        severity,
        "SCOOPC_TEST_DIAGNOSTIC".to_owned(),
        message,
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .unwrap()
}

fn success(
    warnings: Vec<StructuredDiagnosticV1>,
    dumps: Vec<EmittedDumpDescriptorV1>,
) -> Result<ScoopcSuccessV1, ProtocolValidationError> {
    ScoopcSuccessV1::new(
        ProtocolArtifactFingerprint::from_array([1; 32]),
        ProtocolConeIdentity::from_array([2; 32]),
        ProtocolHirFingerprint::from_array([3; 32]),
        ProtocolMirFingerprint::from_array([4; 32]),
        ProtocolLirFingerprint::from_array([5; 32]),
        ProtocolCodeFingerprint::from_array([6; 32]),
        ProtocolRuntimeImageFingerprint::from_array([7; 32]),
        warnings,
        dumps,
    )
}

fn dump() -> EmittedDumpDescriptorV1 {
    EmittedDumpDescriptorV1::new(
        StageDumpKindV1::Hir,
        EmittedDumpDestinationV1::Stdout,
        ProtocolDumpContentDigest::from_array([8; 32]),
    )
}

#[test]
fn response_constructors_enforce_diagnostic_and_dump_shape() {
    let error = diagnostic(DiagnosticSeverityV1::Error, "error".to_owned());
    assert_eq!(
        success(vec![error], Vec::new()).unwrap_err(),
        ProtocolValidationError::SuccessContainsErrorDiagnostic
    );

    assert_eq!(
        success(Vec::new(), vec![dump(), dump()]).unwrap_err(),
        ProtocolValidationError::InvalidDumpDescriptors
    );

    let warning = diagnostic(DiagnosticSeverityV1::Warning, "warning".to_owned());
    assert_eq!(
            ScoopcResponseEnvelopeV1::failure(
                RequestCorrelationId::from_array([0; 16]),
                vec![warning],
            )
            .unwrap_err(),
            ProtocolValidationError::FailureRequiresErrorDiagnostic
        );
}

#[test]
fn response_frames_preserve_complete_diagnostic_text() {
    let warning = diagnostic(DiagnosticSeverityV1::Warning, "detail ".repeat(2_500_000));
    let result = success(vec![warning], Vec::new()).unwrap();
    let response =
        ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([0; 16]), result);
    let frame = crate::encode_response_frame(&response).unwrap();
    assert_eq!(crate::decode_response_frame(&frame).unwrap(), response);
}
