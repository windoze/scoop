mod dumps;

use std::fmt;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use scoop_protocol::{
    DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1, ProtocolValidationError,
    RequestCorrelationId, ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1, ScoopcSuccessV1,
    StageDumpPolicyV1, StructuredDiagnosticV1,
};

const CHILD_COMPILER_FAILURE_EXIT: u8 = 1;
const CHILD_TRANSPORT_FAILURE_EXIT: u8 = 2;
const CHILD_REQUEST_ERROR_CODE: &str = "SCOOPC_CHILD_REQUEST_INVALID";
const CHILD_BUILD_ERROR_CODE: &str = "SCOOPC_BUILD_FAILED";

pub(crate) fn run(version: u32, jobs: std::num::NonZeroUsize) -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    match run_transport(version, stdin.lock(), stdout.lock(), jobs) {
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
    jobs: std::num::NonZeroUsize,
) -> Result<ChildProtocolExit, ChildProtocolError> {
    if version != scoop_protocol::PROTOCOL_VERSION {
        return Err(ChildProtocolError::UnsupportedVersion(version));
    }
    let frame = read_one_frame(&mut input)?;
    let request =
        scoop_protocol::decode_request_frame(&frame).map_err(ChildProtocolError::DecodeRequest)?;
    let response = execute_request(request, jobs)?;
    let exit = match response {
        ScoopcResponseEnvelopeV1::Success { .. } | ScoopcResponseEnvelopeV1::Observed { .. } => {
            ChildProtocolExit::Success
        }
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
    jobs: std::num::NonZeroUsize,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let request_id = request.request_id();
    if request.build().diagnostics() != DiagnosticOutputPolicyV1::Structured {
        return failure_response(
            request_id,
            CHILD_REQUEST_ERROR_CODE,
            "machine builds require structured diagnostics".to_owned(),
        );
    }
    let build = match scoopc::normalize_protocol_build_request(request.build()) {
        Ok(build) => build.with_jobs(jobs),
        Err(error) => {
            return failure_response(request_id, CHILD_REQUEST_ERROR_CODE, error.to_string());
        }
    };
    match build.compile(request.observation_only()) {
        Ok(success) => match success_response(request_id, &success, request.build().emit()) {
            Err(ChildProtocolError::DumpOutput(error)) => failure_response_with_warnings(
                request_id,
                CHILD_BUILD_ERROR_CODE,
                error.to_string(),
                &success
                    .warnings()
                    .structured()
                    .map_err(ChildProtocolError::DiagnosticMapping)?,
            ),
            result => result,
        },
        Err(error) => production_failure_response(request_id, &error),
    }
}

fn production_failure_response(
    request_id: RequestCorrelationId,
    error: &scoopc::SingleConeProductionError,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let diagnostics = error
        .structured_diagnostics()
        .map_err(ChildProtocolError::DiagnosticMapping)?;
    ScoopcResponseEnvelopeV1::failure(request_id, diagnostics)
        .map_err(ChildProtocolError::ConstructResponse)
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
    warnings: &[StructuredDiagnosticV1],
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
    diagnostics.extend_from_slice(warnings);
    ScoopcResponseEnvelopeV1::failure(request_id, diagnostics)
        .map_err(ChildProtocolError::ConstructResponse)
}

fn success_response(
    request_id: RequestCorrelationId,
    success: &scoopc::SingleConeCompilation,
    policy: &StageDumpPolicyV1,
) -> Result<ScoopcResponseEnvelopeV1, ChildProtocolError> {
    let dumps = dumps::write_dumps(policy, success.emitted_dumps())
        .map_err(ChildProtocolError::DumpOutput)?;
    let warnings = success
        .warnings()
        .structured()
        .map_err(ChildProtocolError::DiagnosticMapping)?;
    match success {
        scoopc::SingleConeCompilation::Published(success) => {
            let artifact = Box::new(success.artifact().summary().manifest_summary().clone());
            let product = match success.documentation() {
                Some(documentation) => scoop_protocol::ScoopcProductV1::ArtifactWithDocumentation {
                    artifact,
                    documentation: documentation_response(documentation)?,
                },
                None => scoop_protocol::ScoopcProductV1::Artifact(artifact),
            };
            let result = ScoopcSuccessV1::new(product, warnings, dumps)
                .map_err(ChildProtocolError::ConstructResponse)?
                .with_used_generators(success.used_generators().to_vec())
                .with_compiler_inputs(success.compiler_inputs().to_vec());
            Ok(ScoopcResponseEnvelopeV1::success(request_id, result))
        }
        scoopc::SingleConeCompilation::Documented(success) => {
            let result = ScoopcSuccessV1::new(
                scoop_protocol::ScoopcProductV1::Documentation(documentation_response(
                    success.documentation(),
                )?),
                warnings,
                dumps,
            )
            .map_err(ChildProtocolError::ConstructResponse)?
            .with_used_generators(success.used_generators().to_vec())
            .with_compiler_inputs(success.compiler_inputs().to_vec());
            Ok(ScoopcResponseEnvelopeV1::success(request_id, result))
        }
        scoopc::SingleConeCompilation::Checked(success) => {
            let result =
                ScoopcSuccessV1::new(scoop_protocol::ScoopcProductV1::Check, warnings, dumps)
                    .map_err(ChildProtocolError::ConstructResponse)?
                    .with_used_generators(success.used_generators().to_vec())
                    .with_compiler_inputs(success.compiler_inputs().to_vec());
            Ok(ScoopcResponseEnvelopeV1::success(request_id, result))
        }
        scoopc::SingleConeCompilation::Observed(_) => Ok(ScoopcResponseEnvelopeV1::Observed {
            request_id,
            result: scoop_protocol::ScoopcObservationV1::new(warnings, dumps)
                .map_err(ChildProtocolError::ConstructResponse)?,
        }),
    }
}

fn documentation_response(
    output: &scoopc::PublishedDocumentation,
) -> Result<scoop_protocol::ScoopcDocumentationOutputV1, ChildProtocolError> {
    let path = scoop_protocol::HostPathCarrier::from_path(output.path()).map_err(|error| {
        ChildProtocolError::ConstructResponse(ProtocolValidationError::HostPath(error))
    })?;
    scoop_protocol::ScoopcDocumentationOutputV1::new(
        path,
        output
            .warnings()
            .structured()
            .map_err(ChildProtocolError::DiagnosticMapping)?,
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
    DumpOutput(io::Error),
    DiagnosticMapping(scoopc::DiagnosticMappingError),
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
            Self::DiagnosticMapping(error) => error.fmt(formatter),
            Self::DumpOutput(error) => write!(formatter, "cannot write stage dumps: {error}"),
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
            Self::UnsupportedVersion(_) => None,
            Self::DumpOutput(error) => Some(error),
            Self::DiagnosticMapping(error) => Some(error),
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
                CurrentConeRequestV1::SingleFile {
                    source: host_path("project/main.scoop"),
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
        let warning = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Warning,
            "SCOOPC_COMPILER_WARNING".to_owned(),
            "retained catch-all warning".to_owned(),
            DiagnosticOriginV1::SemanticSourceSpan {
                cone: scoop_protocol::ProtocolConeIdentity::from_array([17; 32]),
                logical_path: scoop_identity::NormalizedSourcePath::new("src/Warning.scoop")
                    .unwrap(),
                span: scoop_protocol::ProtocolByteSpan::new(4, 9).unwrap(),
            },
            Vec::new(),
        )
        .unwrap();
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
        assert_eq!(diagnostics[1].code(), "SCOOPC_COMPILER_WARNING");
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
            std::num::NonZeroUsize::MIN,
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
                std::num::NonZeroUsize::MIN,
            ),
            Err(ChildProtocolError::DecodeRequest(_))
        ));
        assert!(response.is_empty());
    }
}
