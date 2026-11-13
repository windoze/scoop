use super::*;
use crate::{DiagnosticOriginV1, DiagnosticSeverityV1};

mod check;
mod documentation;

#[test]
fn feedback_summary_and_build_file_observations_round_trip() {
    let summary = scoop_profile::UseSummary {
        matched_functions: 2,
        matched_positions: 9,
        partial_positions: 1,
        missing_positions: 3,
        changed_positions: 4,
        consumers: [
            (scoop_profile::FeedbackConsumer::Inline, 2),
            (scoop_profile::FeedbackConsumer::StringFusion, 1),
        ]
        .into(),
    };
    let result = success(vec![], vec![])
        .unwrap()
        .with_profile_use(Some(summary))
        .with_compiler_inputs(vec![crate::CompilerInputV1::BuildFile(
            crate::BuildFileInputV1 {
                path: HostPathCarrier::from_path(std::path::Path::new(
                    "training 数据/counts.profdata",
                ))
                .unwrap(),
                length: 123,
                modified: (false, 55, 42),
            },
        )]);
    let response =
        ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([12; 16]), result);
    let frame = crate::encode_response_frame(&response).unwrap();
    assert_eq!(crate::decode_response_frame(&frame).unwrap(), response);
}

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
    ScoopcSuccessV1::new(crate::test_summary(), warnings, dumps)
}

fn dump() -> EmittedDumpDescriptorV1 {
    EmittedDumpDescriptorV1::new(StageDumpKindV1::Hir, EmittedDumpDestinationV1::Stdout)
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

#[test]
fn file_level_diagnostic_keeps_its_path_without_a_byte_span() {
    let origin = DiagnosticOriginV1::HostPath {
        path: HostPathCarrier::from_path(std::path::Path::new("/workspace/bad.scoop")).unwrap(),
    };
    let diagnostic = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOPC_BUILD_FAILED".to_owned(),
        "source file is not valid UTF-8".to_owned(),
        origin,
        Vec::new(),
    )
    .unwrap();
    let bytes = scoop_wire::encode(&diagnostic).unwrap();
    let decoded: StructuredDiagnosticV1 = scoop_wire::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, diagnostic);
}

#[test]
fn observation_response_needs_no_artifact_summary() {
    let response = ScoopcResponseEnvelopeV1::Observed {
        request_id: RequestCorrelationId::from_array([3; 16]),
        result: ScoopcObservationV1::new(
            Vec::new(),
            vec![EmittedDumpDescriptorV1::new(
                StageDumpKindV1::Ast,
                EmittedDumpDestinationV1::File(
                    HostPathCarrier::from_path(std::path::Path::new("/tmp/ast.txt")).unwrap(),
                ),
            )],
        )
        .unwrap(),
    };
    assert_eq!(
        crate::decode_response_frame(&crate::encode_response_frame(&response).unwrap()).unwrap(),
        response
    );
}

#[test]
fn response_preserves_profiling_output_path() {
    let path =
        HostPathCarrier::from_path(std::path::Path::new("output.slib.profile.json")).unwrap();
    let result = success(Vec::new(), Vec::new())
        .unwrap()
        .with_profile_map(Some(path));
    let response =
        ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([4; 16]), result);
    assert_eq!(
        crate::decode_response_frame(&crate::encode_response_frame(&response).unwrap()).unwrap(),
        response
    );
}
