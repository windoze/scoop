use std::fmt;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use scoop_protocol::{
    DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
    ProtocolArtifactFingerprint, ProtocolCodeFingerprint, ProtocolConeIdentity,
    ProtocolHirFingerprint, ProtocolLirFingerprint, ProtocolMirFingerprint,
    ProtocolRuntimeImageFingerprint, ProtocolValidationError, RequestCorrelationId,
    ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1, ScoopcSuccessV1, StageDumpPolicyV1,
    StructuredDiagnosticV1,
};
use scoop_slib::FingerprintAvailability;

const CHILD_COMPILER_FAILURE_EXIT: u8 = 1;
const CHILD_TRANSPORT_FAILURE_EXIT: u8 = 2;
const CHILD_REQUEST_ERROR_CODE: &str = "SCOOPC_CHILD_REQUEST_INVALID";
const CHILD_BUILD_ERROR_CODE: &str = "SCOOPC_BUILD_FAILED";
const CHILD_WARNING_CODE: &str = "SCOOPC_COMPILER_WARNING";

pub(crate) fn run(version: u32) -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_transport(version, stdin.lock(), stdout.lock()) {
        Ok(ChildProtocolExit::Success) => ExitCode::SUCCESS,
        Ok(ChildProtocolExit::CompilerFailure) => ExitCode::from(CHILD_COMPILER_FAILURE_EXIT),
        Err(error) => {
            eprintln!("scoopc child transport failed: {error}");
            ExitCode::from(CHILD_TRANSPORT_FAILURE_EXIT)
        }
    }
}

fn run_transport(
    version: u32,
    mut input: impl Read,
    mut output: impl Write,
) -> Result<ChildProtocolExit, ChildProtocolError> {
    if version != scoop_protocol::PROTOCOL_VERSION {
        return Err(ChildProtocolError::UnsupportedVersion(version));
    }
    let frame = read_one_frame(&mut input)?;
    let request =
        scoop_protocol::decode_request_frame(&frame).map_err(ChildProtocolError::DecodeRequest)?;
    let response = execute_request(request)?;
    let exit = match response {
        ScoopcResponseEnvelopeV1::Success { .. } => ChildProtocolExit::Success,
        ScoopcResponseEnvelopeV1::Failure { .. } => ChildProtocolExit::CompilerFailure,
    };
    let frame = scoop_protocol::encode_response_frame(&response)
        .map_err(ChildProtocolError::EncodeResponse)?;
    output
        .write_all(&frame)
        .and_then(|()| output.flush())
        .map_err(ChildProtocolError::WriteResponse)?;
    Ok(exit)
}

fn read_one_frame(input: &mut impl Read) -> Result<Vec<u8>, ChildProtocolError> {
    let mut frame = Vec::new();
    input
        .read_to_end(&mut frame)
        .map_err(ChildProtocolError::ReadRequest)?;
    Ok(frame)
}

fn execute_request(
    request: ScoopcRequestEnvelopeV1,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let request_id = request.request_id();
    if request.build().diagnostics() != DiagnosticOutputPolicyV1::Structured
        || request.build().emit() != StageDumpPolicyV1::None
    {
        return failure_response(
            request_id,
            CHILD_REQUEST_ERROR_CODE,
            "machine builds require structured diagnostics and emit=None".to_owned(),
        );
    }
    let build = match scoopc::normalize_protocol_build_request(request.build()) {
        Ok(build) => build,
        Err(error) => {
            return failure_response(request_id, CHILD_REQUEST_ERROR_CODE, error.to_string());
        }
    };
    match build.build_and_publish() {
        Ok(success) => success_response(request_id, &success),
        Err(error) => production_failure_response(request_id, &error),
    }
}

fn production_failure_response(
    request_id: RequestCorrelationId,
    error: &scoopc::SingleConeProductionError,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let warnings = error
        .warnings()
        .map(|warnings| warnings.diagnostics())
        .unwrap_or_default();
    failure_response_with_warnings(
        request_id,
        CHILD_BUILD_ERROR_CODE,
        error.to_string(),
        warnings,
    )
}

fn failure_response(
    request_id: RequestCorrelationId,
    code: &'static str,
    message: String,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    failure_response_with_warnings(request_id, code, message, &[])
}

fn failure_response_with_warnings(
    request_id: RequestCorrelationId,
    code: &'static str,
    message: String,
    warnings: &[scoop_ast::Diagnostic],
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let diagnostic = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        code.to_owned(),
        message,
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .map_err(ChildProtocolError::ConstructResponse)?;
    let mut diagnostics = vec![diagnostic];
    diagnostics.extend(
        warnings
            .iter()
            .map(protocol_warning)
            .collect::<Result<Vec<_>, _>>()?,
    );
    ScoopcResponseEnvelopeV1::failure(request_id, diagnostics)
        .map_err(ChildProtocolError::ConstructResponse)
}

fn success_response(
    request_id: RequestCorrelationId,
    success: &scoopc::SingleConeProductionSuccess,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    if success.emitted_dump().is_some() {
        return Err(ChildProtocolError::UnexpectedDump);
    }
    let artifact = success.artifact().validation();
    let semantic = artifact.compile_summary().semantic_fingerprints();
    let FingerprintAvailability::Available(code) = semantic.code() else {
        return Err(ChildProtocolError::MissingStrongFingerprint("code"));
    };
    let FingerprintAvailability::Available(runtime_image) = semantic.runtime_image() else {
        return Err(ChildProtocolError::MissingStrongFingerprint(
            "runtime image",
        ));
    };
    let warnings = success
        .warnings()
        .diagnostics()
        .iter()
        .map(protocol_warning)
        .collect::<Result<Vec<_>, _>>()?;
    let result = ScoopcSuccessV1::new(
        ProtocolArtifactFingerprint::from_array(*artifact.artifact_fingerprint().as_array()),
        ProtocolConeIdentity::from_array(*artifact.identity().as_array()),
        ProtocolHirFingerprint::from_array(*semantic.hir().as_array()),
        ProtocolMirFingerprint::from_array(*semantic.mir().as_array()),
        ProtocolLirFingerprint::from_array(*semantic.lir().as_array()),
        ProtocolCodeFingerprint::from_array(*code.as_array()),
        ProtocolRuntimeImageFingerprint::from_array(*runtime_image.as_array()),
        warnings,
        Vec::new(),
    )
    .map_err(ChildProtocolError::ConstructResponse)?;
    Ok(ScoopcResponseEnvelopeV1::success(request_id, result))
}

fn protocol_warning(
    warning: &scoop_ast::Diagnostic,
) -> Result<StructuredDiagnosticV1, ChildProtocolError> {
    let notes = warning
        .notes
        .iter()
        .map(|note| {
            DiagnosticNoteV1::new(note.message.clone(), DiagnosticOriginV1::None)
                .map_err(ChildProtocolError::ConstructResponse)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredDiagnosticV1::new(
        match warning.severity {
            scoop_ast::DiagnosticSeverity::Warning => DiagnosticSeverityV1::Warning,
            scoop_ast::DiagnosticSeverity::Error => DiagnosticSeverityV1::Error,
        },
        CHILD_WARNING_CODE.to_owned(),
        warning.message.clone(),
        DiagnosticOriginV1::None,
        notes,
    )
    .map_err(ChildProtocolError::ConstructResponse)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildProtocolExit {
    Success,
    CompilerFailure,
}

#[derive(Debug)]
enum ChildProtocolError {
    UnsupportedVersion(u32),
    ReadRequest(io::Error),
    DecodeRequest(scoop_protocol::ProtocolReadError),
    ConstructResponse(ProtocolValidationError),
    MissingStrongFingerprint(&'static str),
    UnexpectedDump,
    EncodeResponse(scoop_protocol::ProtocolWriteError),
    WriteResponse(io::Error),
}

impl fmt::Display for ChildProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported child protocol version {version}")
            }
            Self::ReadRequest(source) => write!(formatter, "cannot read request frame: {source}"),
            Self::DecodeRequest(source) => write!(formatter, "invalid request frame: {source}"),
            Self::ConstructResponse(source) => {
                write!(formatter, "cannot construct response: {source}")
            }
            Self::MissingStrongFingerprint(kind) => {
                write!(
                    formatter,
                    "successful strong artifact has no {kind} fingerprint"
                )
            }
            Self::UnexpectedDump => {
                formatter.write_str("machine build unexpectedly produced a stage dump")
            }
            Self::EncodeResponse(source) => write!(formatter, "cannot encode response: {source}"),
            Self::WriteResponse(source) => write!(formatter, "cannot write response: {source}"),
        }
    }
}

impl std::error::Error for ChildProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadRequest(source) | Self::WriteResponse(source) => Some(source),
            Self::DecodeRequest(source) => Some(source),
            Self::ConstructResponse(source) => Some(source),
            Self::EncodeResponse(source) => Some(source),
            Self::UnsupportedVersion(_)
            | Self::MissingStrongFingerprint(_)
            | Self::UnexpectedDump => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;

    use scoop_protocol::{
        CurrentConeRequestV1, HostPathCarrier, ScoopcBuildRequestV1, TargetSelectionRequestV1,
        TrustedCoreRequestV1,
    };

    use super::*;

    fn host_path(path: &str) -> HostPathCarrier {
        HostPathCarrier::from_path(Path::new(path)).unwrap()
    }

    fn non_machine_request() -> ScoopcRequestEnvelopeV1 {
        ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_array([9; 16]),
            ScoopcBuildRequestV1::new(
                CurrentConeRequestV1::ManifestRoot {
                    root: host_path("project/Cone.toml"),
                },
                Vec::new(),
                Vec::new(),
                TrustedCoreRequestV1::ArtifactSlot {
                    artifact: host_path("sysroot/core.slib"),
                },
                TargetSelectionRequestV1::new("aarch64-apple-darwin".to_owned()).unwrap(),
                host_path("output/current.slib"),
                DiagnosticOutputPolicyV1::Human,
                StageDumpPolicyV1::None,
            )
            .unwrap(),
        )
    }

    #[test]
    fn failure_response_roundtrips_warnings_as_separate_typed_diagnostics() {
        let request_id = non_machine_request().request_id();
        let warning = scoop_ast::Diagnostic::warning_at(
            scoop_ast::Span::new(4, 9),
            "retained catch-all warning",
        );
        let response = failure_response_with_warnings(
            request_id,
            CHILD_BUILD_ERROR_CODE,
            "publication failed".to_owned(),
            &[warning],
        )
        .unwrap();
        let frame = scoop_protocol::encode_response_frame(&response).unwrap();
        let ScoopcResponseEnvelopeV1::Failure {
            request_id: actual,
            diagnostics,
        } = scoop_protocol::decode_response_frame(&frame).unwrap()
        else {
            panic!("warnings must not turn the failed build into a success");
        };
        assert_eq!(actual, request_id);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity(), DiagnosticSeverityV1::Error);
        assert_eq!(diagnostics[0].message(), "publication failed");
        assert_eq!(diagnostics[1].severity(), DiagnosticSeverityV1::Warning);
        assert_eq!(diagnostics[1].code(), CHILD_WARNING_CODE);
        assert_eq!(diagnostics[1].message(), "retained catch-all warning");
    }

    #[test]
    fn transport_emits_one_correlated_failure_frame() {
        let request = non_machine_request();
        let request_frame = scoop_protocol::encode_request_frame(&request).unwrap();
        let mut response_frame = Vec::new();

        let exit = run_transport(
            scoop_protocol::PROTOCOL_VERSION,
            Cursor::new(request_frame),
            &mut response_frame,
        )
        .unwrap();

        assert_eq!(exit, ChildProtocolExit::CompilerFailure);
        let response = scoop_protocol::decode_response_frame(&response_frame).unwrap();
        let ScoopcResponseEnvelopeV1::Failure {
            request_id,
            diagnostics,
        } = response
        else {
            panic!("non-machine policy must produce a failure response")
        };
        assert_eq!(request_id, request.request_id());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code(), CHILD_REQUEST_ERROR_CODE);
    }

    #[test]
    fn transport_rejects_trailing_frames_without_a_response() {
        let mut trailing = scoop_protocol::encode_request_frame(&non_machine_request()).unwrap();
        trailing.push(0);
        let mut response = Vec::new();
        assert!(matches!(
            run_transport(
                scoop_protocol::PROTOCOL_VERSION,
                Cursor::new(trailing),
                &mut response,
            ),
            Err(ChildProtocolError::DecodeRequest(_))
        ));
        assert!(response.is_empty());
    }
}
